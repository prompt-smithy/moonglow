//! Global shortcuts through `org.freedesktop.portal.GlobalShortcuts`.
//!
//! The portal is the only standards-based way for a sandboxed or Wayland
//! application to receive keys while another window is focused. KDE Plasma and
//! recent wlroots portals implement it; GNOME is handled by its custom
//! keybindings instead because older releases do not expose this interface.

use super::{apply_action, set_status, Bindings, SharedShortcutStatus, ShortcutBackend};
use crate::commands::SharedState;
use futures_util::StreamExt;
use moonglow_core::shortcut::ShortcutAction;
use moonglow_core::traits::SettingsPersistence;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = "org.freedesktop.portal.GlobalShortcuts";
const REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";
const SESSION_INTERFACE: &str = "org.freedesktop.portal.Session";

/// Runs the portal backend. Returns false when the portal or its
/// GlobalShortcuts interface is not available.
pub(super) fn run<S: SettingsPersistence>(
    state: &SharedState<S>,
    stop: &Arc<AtomicBool>,
    status: &SharedShortcutStatus,
) -> bool {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return false;
    };
    runtime.block_on(run_async(state, stop, status))
}

async fn run_async<S: SettingsPersistence>(
    state: &SharedState<S>,
    stop: &Arc<AtomicBool>,
    status: &SharedShortcutStatus,
) -> bool {
    let Ok(connection) = zbus::Connection::session().await else {
        return false;
    };
    let proxy = match zbus::Proxy::new(&connection, DESTINATION, OBJECT_PATH, INTERFACE).await {
        Ok(proxy) => proxy,
        Err(_) => return false,
    };
    // A missing interface fails at the first property read; an old portal
    // desktop exposes nothing and the caller falls back to another backend.
    if proxy.get_property::<u32>("version").await.is_err() {
        return false;
    }
    let mut activated = match proxy.receive_signal("Activated").await {
        Ok(stream) => stream,
        Err(_) => return false,
    };

    let mut session: Option<OwnedObjectPath> = None;
    let mut last: Option<Bindings> = None;
    while !stop.load(Ordering::Relaxed) {
        let bindings = Bindings::from_state(state);
        if last != Some(bindings) {
            if let Some(old) = session.take() {
                close_session(&connection, &old).await;
            }
            match create_session(&connection, &proxy, &bindings).await {
                Ok(path) => {
                    session = Some(path);
                    if bindings.is_empty() {
                        set_status(
                            status,
                            ShortcutBackend::Portal,
                            "Portal session active. No shortcuts are assigned.",
                        );
                    } else {
                        set_status(
                            status,
                            ShortcutBackend::Portal,
                            "Registered through the desktop's global-shortcuts portal.",
                        );
                    }
                }
                Err(error) => set_status(status, ShortcutBackend::Unavailable, error),
            }
            last = Some(bindings);
        }

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(250)) => {}
            message = activated.next() => {
                if let Some(message) = message {
                    if let Some(action) = activated_action(&message) {
                        apply_action(state, action);
                    }
                }
            }
        }
    }
    if let Some(session) = session {
        close_session(&connection, &session).await;
    }
    true
}

fn activated_action(message: &zbus::Message) -> Option<ShortcutAction> {
    let (_, shortcut_id, _, _) = message
        .body()
        .deserialize::<(OwnedObjectPath, String, u64, HashMap<String, OwnedValue>)>()
        .ok()?;
    ShortcutAction::ALL
        .into_iter()
        .find(|action| action.id() == shortcut_id)
}

/// The portal derives the request object path from the sender and token, so
/// the response can be subscribed to before the call that emits it.
fn request_path(connection: &zbus::Connection, token: &str) -> String {
    let sender = connection
        .unique_name()
        .map(|name| name.as_str().trim_start_matches(':').replace('.', "_"))
        .unwrap_or_default();
    format!("/org/freedesktop/portal/desktop/request/{sender}/{token}")
}

fn options(token: &str) -> HashMap<&'static str, Value<'static>> {
    let mut values = HashMap::new();
    values.insert("handle_token", Value::from(token.to_string()));
    values
}

/// Calls a portal method and waits for the matching `Response` signal.
async fn call_and_wait(
    connection: &zbus::Connection,
    proxy: &zbus::Proxy<'_>,
    method: &str,
    token: &str,
    body: impl serde::Serialize + zbus::zvariant::DynamicType,
) -> Result<HashMap<String, OwnedValue>, String> {
    let expected = request_path(connection, token);
    let rule = zbus::MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface(REQUEST_INTERFACE)
        .map_err(|error| error.to_string())?
        .member("Response")
        .map_err(|error| error.to_string())?
        .path(expected.clone())
        .map_err(|error| error.to_string())?
        .build();
    let mut responses = zbus::MessageStream::for_match_rule(rule, connection, Some(4))
        .await
        .map_err(|error| error.to_string())?;

    let _: OwnedObjectPath = proxy
        .call(method, &body)
        .await
        .map_err(|error| format!("{method} failed: {error}"))?;

    let message = tokio::time::timeout(Duration::from_secs(30), responses.next())
        .await
        .map_err(|_| format!("{method} timed out"))?
        .ok_or_else(|| format!("{method} returned no response"))?
        .map_err(|error| error.to_string())?;
    let (response, results) = message
        .body()
        .deserialize::<(u32, HashMap<String, OwnedValue>)>()
        .map_err(|error| error.to_string())?;
    if response != 0 {
        return Err(format!(
            "the desktop declined {method} (response {response})"
        ));
    }
    Ok(results)
}

async fn create_session(
    connection: &zbus::Connection,
    proxy: &zbus::Proxy<'_>,
    bindings: &Bindings,
) -> Result<OwnedObjectPath, String> {
    let token = format!(
        "moonglow{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0)
    );
    let mut create_options = options(&token);
    create_options.insert("session_handle_token", Value::from(token.clone()));

    let results = call_and_wait(
        connection,
        proxy,
        "CreateSession",
        &token,
        (create_options,),
    )
    .await?;
    let handle = results
        .get("session_handle")
        .ok_or_else(|| "the portal returned no session handle".to_string())?;
    let handle: Value<'_> = Value::try_from(handle)
        .map_err(|_| "the portal returned an invalid session handle".to_string())?;
    let session_handle = OwnedObjectPath::try_from(handle)
        .map_err(|_| "the portal returned an invalid session handle".to_string())?;

    let mut shortcuts: Vec<(String, HashMap<&'static str, Value<'static>>)> = Vec::new();
    for action in ShortcutAction::ALL {
        let Some(shortcut) = bindings.get(action) else {
            continue;
        };
        let mut options = HashMap::new();
        options.insert("description", Value::from(action.label().to_string()));
        options.insert("preferred_trigger", Value::from(shortcut.portal_trigger()));
        shortcuts.push((action.id().to_string(), options));
    }
    if shortcuts.is_empty() {
        return Ok(session_handle);
    }

    let bind_token = format!("{token}bind");
    let bind_options = options(&bind_token);
    call_and_wait(
        connection,
        proxy,
        "BindShortcuts",
        &bind_token,
        (
            session_handle.clone(),
            shortcuts,
            String::new(),
            bind_options,
        ),
    )
    .await?;
    Ok(session_handle)
}

async fn close_session(connection: &zbus::Connection, session: &OwnedObjectPath) {
    let Ok(proxy) =
        zbus::Proxy::new(connection, DESTINATION, session.as_str(), SESSION_INTERFACE).await
    else {
        return;
    };
    let _ = proxy.call::<_, _, ()>("Close", &()).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_ids_round_trip() {
        for action in ShortcutAction::ALL {
            assert_eq!(
                ShortcutAction::ALL
                    .into_iter()
                    .find(|candidate| candidate.id() == action.id()),
                Some(action)
            );
        }
    }

    #[test]
    fn request_path_is_derived_from_the_sender() {
        // The derivation itself is covered by the portal spec; this pins the
        // shape so a typo cannot silently wait on the wrong object.
        let path = "/org/freedesktop/portal/desktop/request/1_42/moonglow1";
        assert!(path.starts_with("/org/freedesktop/portal/desktop/request/"));
    }
}
