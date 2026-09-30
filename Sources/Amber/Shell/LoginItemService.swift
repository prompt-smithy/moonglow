import ServiceManagement
import AmberKit

/// ``LoginItemService`` backed by `SMAppService`.
///
/// `SMAppService.mainApp` registers *the running bundle*, so this only does anything from a
/// packaged `Night Mode Controller.app`. Launched straight out of the build directory — `swift run`,
/// or the bare binary under `.build` — there is no bundle for the system to register and every call reports
/// failure. That is surfaced rather than swallowed: the settings toggle reverts, so it never
/// claims a registration the system did not actually make.
final class SMAppServiceLoginItemService: LoginItemService {

    var isEnabled: Bool {
        guard #available(macOS 13.0, *) else { return false }
        return SMAppService.mainApp.status == .enabled
    }

    var requiresApproval: Bool {
        guard #available(macOS 13.0, *) else { return false }
        return SMAppService.mainApp.status == .requiresApproval
    }

    @discardableResult
    func setEnabled(_ enabled: Bool) -> Bool {
        guard #available(macOS 13.0, *) else { return false }
        let service = SMAppService.mainApp

        // `register()` throws if the item is already registered, so the no-op cases are handled
        // before it rather than by catching.
        if enabled, service.status == .enabled { return true }
        if !enabled, service.status == .notRegistered { return true }

        do {
            if enabled {
                try service.register()
            } else {
                try service.unregister()
            }
        } catch {
            return false
        }

        // `register()` succeeds even when macOS then parks the request as `.requiresApproval`, so
        // success has to be judged from the resulting status and not from the absence of a throw.
        // An approval-pending registration still counts as accepted — it is on, it just has not
        // taken effect yet, which `requiresApproval` reports separately.
        return enabled ? service.status != .notRegistered : service.status != .enabled
    }

    func openLoginItemsSettings() {
        guard #available(macOS 13.0, *) else { return }
        SMAppService.openSystemSettingsLoginItems()
    }
}
