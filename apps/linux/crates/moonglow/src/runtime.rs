//! Blocking bridge for backend D-Bus calls from any thread.
//!
//! Backend `apply` methods are synchronous but speak D-Bus through a tokio
//! runtime. Tray menu handlers run *inside* a tokio runtime already, where a
//! direct `block_on` panics ("Cannot block_on within a runtime") — killing
//! the reply (the host sees a timeout) and poisoning any mutex held across
//! the call. Hopping to a fresh OS thread keeps every call site safe no
//! matter which thread it runs on.
//!
//! Rule: any backend call reachable from tray handlers (or of otherwise
//! unknown provenance) goes through [`drive`]. Threads that are plain OS
//! threads by construction (startup probes, dedicated refreshers) may block
//! their runtime directly.

use std::future::Future;

/// Drives `future` on `runtime`, callable from any thread — including threads
/// already inside a tokio runtime, where direct `block_on` panics.
pub fn drive<F>(runtime: &tokio::runtime::Runtime, future: F) -> F::Output
where
    F: Future + Send,
    F::Output: Send,
{
    std::thread::scope(|scope| {
        scope
            .spawn(|| runtime.block_on(future))
            .join()
            .expect("dbus worker thread panicked")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn drive_survives_nested_runtime_context() {
        // A direct `block_on` here panics; the hop must not.
        let outer = test_runtime();
        let inner = test_runtime();
        let value = outer.block_on(async { drive(&inner, async { 7 }) });
        assert_eq!(value, 7);
    }

    #[test]
    fn drive_works_from_plain_threads() {
        let runtime = test_runtime();
        assert_eq!(drive(&runtime, async { 42 }), 42);
    }
}
