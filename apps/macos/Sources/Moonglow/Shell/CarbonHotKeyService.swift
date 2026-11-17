import Carbon
import MoonglowKit

/// Global hot keys through Carbon's `RegisterEventHotKey`.
///
/// Carbon needs no Accessibility permission — unlike event taps or global monitors —
/// which is why it is still the right tool for a handful of app hot keys. Delivery runs
/// on the main thread, so actions call the main-actor view model directly.
///
/// Main-thread confined by contract: registration happens from the main-actor view
/// model and delivery fires on the main run loop. That is what makes the unchecked
/// conformance below sound.
final class CarbonHotKeyService: HotKeyService, @unchecked Sendable {

    /// FourCC stamp on our registrations. Any value unique to the app works.
    private static let signature = OSType(0x4D474C57) // "MGLW"

    /// Action handlers, set by the composition root. Consulted at press time so the
    /// registrations below never go stale.
    var handlers: [HotKeyAction: @MainActor () -> Void] = [:]

    private var registrations: [EventHotKeyRef] = []
    private var eventHandler: EventHandlerRef?
    private var assignments: [(action: HotKeyAction, shortcut: HotKeyShortcut?)] = []
    private var restoreBrightnessSuspendedForMenu = false

    @discardableResult
    func updateHotKeys(_ assignments: [(action: HotKeyAction, shortcut: HotKeyShortcut?)]) -> Set<HotKeyAction> {
        self.assignments = assignments
        unregisterAll()
        guard installHandlerIfNeeded() else { return [] }
        var registeredActions = Set<HotKeyAction>()
        for assignment in assignments {
            if restoreBrightnessSuspendedForMenu && assignment.action == .restoreBrightness { continue }
            guard let shortcut = assignment.shortcut, shortcut.isValid else { continue }
            var reference: EventHotKeyRef?
            let identifier = EventHotKeyID(signature: Self.signature, id: assignment.action.rawValue)
            let status = RegisterEventHotKey(
                shortcut.keyCode,
                shortcut.modifiers,
                identifier,
                GetApplicationEventTarget(),
                0,
                &reference
            )
            guard status == noErr, let reference else {
                AppLog.shared.log(
                    .lifecycle,
                    "hot key \(shortcut.displayText) refused (OSStatus \(status))"
                )
                continue
            }
            registrations.append(reference)
            registeredActions.insert(assignment.action)
        }
        return registeredActions
    }

    /// Lets the tracking status menu receive the recovery key as its own key equivalent.
    /// Carbon hot keys are delivered on the app event loop, which may be occupied while
    /// AppKit tracks an open menu.
    func suspendRestoreBrightnessForMenu() {
        guard !restoreBrightnessSuspendedForMenu else { return }
        restoreBrightnessSuspendedForMenu = true
        _ = updateHotKeys(assignments)
    }

    /// Re-registers the recovery key after the menu closes. The view model performs the
    /// registration update so it can verify success and re-lock brightness if needed.
    func resumeRestoreBrightnessAfterMenu() {
        restoreBrightnessSuspendedForMenu = false
    }

    private func installHandlerIfNeeded() -> Bool {
        guard eventHandler == nil else { return true }
        var eventType = EventTypeSpec(
            eventClass: OSType(kEventClassKeyboard),
            eventKind: UInt32(kEventHotKeyPressed)
        )
        let status = InstallEventHandler(
            GetApplicationEventTarget(),
            carbonHotKeyHandler,
            1,
            &eventType,
            Unmanaged.passUnretained(self).toOpaque(),
            &eventHandler
        )
        guard status == noErr, eventHandler != nil else {
            AppLog.shared.log(.lifecycle, "hot-key event handler install failed (OSStatus \(status))")
            return false
        }
        return true
    }

    @MainActor
    fileprivate func handle(action: HotKeyAction) {
        handlers[action]?()
    }

    private func unregisterAll() {
        for registration in registrations {
            UnregisterEventHotKey(registration)
        }
        registrations.removeAll()
    }

    deinit {
        unregisterAll()
        if let eventHandler {
            RemoveEventHandler(eventHandler)
        }
    }
}

/// Routes a Carbon hot-key press to its action.
private func carbonHotKeyHandler(
    nextHandler: EventHandlerCallRef?,
    event: EventRef?,
    userData: UnsafeMutableRawPointer?
) -> OSStatus {
    guard let event, let userData else { return OSStatus(eventNotHandledErr) }

    var identifier = EventHotKeyID()
    let status = GetEventParameter(
        event,
        EventParamName(kEventParamDirectObject),
        EventParamType(typeEventHotKeyID),
        nil,
        MemoryLayout<EventHotKeyID>.size,
        nil,
        &identifier
    )
    guard status == noErr, let action = HotKeyAction(rawValue: identifier.id) else {
        return OSStatus(eventNotHandledErr)
    }

    // Installed on the application target, which Carbon drains on the main thread.
    let service = Unmanaged<CarbonHotKeyService>.fromOpaque(userData).takeUnretainedValue()
    MainActor.assumeIsolated {
        service.handle(action: action)
    }
    return noErr
}
