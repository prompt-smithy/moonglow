import AppKit

/// A window that hides instead of closing, and hides itself when left alone.
///
/// Ported from `Views/MainWindow.axaml.cs`: closing the settings window keeps the app alive in the
/// menu bar, and ten seconds without interaction puts the window away on its own.
final class HidingWindow: NSWindow {

    /// Set to `true` for the real shutdown path so quit can actually close the window.
    var allowClose = false

    /// Matches `InactivityTimeout` in the original.
    private static let inactivityTimeout: TimeInterval = 10

    private var inactivityTimer: Timer?
    private var activityMonitor: Any?

    /// Shows the window and arms the countdown. Mirrors `ShowWithInactivityTimeout`.
    func showWithInactivityTimeout() {
        // Mouse-moved events are only generated when the window asks for them, and moving the
        // pointer counts as activity.
        acceptsMouseMovedEvents = true

        NSApp.activate(ignoringOtherApps: true)
        makeKeyAndOrderFront(nil)
        startMonitoringActivity()
        restartInactivityTimer()
    }

    override func close() {
        if allowClose {
            super.close()
        } else {
            hide()
        }
    }

    override func orderOut(_ sender: Any?) {
        super.orderOut(sender)
        stopInactivity()
    }

    // MARK: - Inactivity

    private func hide() {
        stopInactivity()
        orderOut(nil)
    }

    /// Counts interaction anywhere in the app, not just in this window.
    ///
    /// Choosing a value from a drop-down opens a window of its own, so those clicks never reach this
    /// window — counting only its own events would hide the settings window mid-choice.
    private func startMonitoringActivity() {
        stopMonitoringActivity()

        activityMonitor = NSEvent.addLocalMonitorForEvents(matching: .any) { [weak self] event in
            MainActor.assumeIsolated {
                self?.restartInactivityTimer()
            }
            return event
        }
    }

    private func restartInactivityTimer() {
        guard isVisible else { return }

        // One timer, reused: this runs per event, including mouse-moved at display
        // refresh rates, and allocating a timer each time is pure churn.
        if let inactivityTimer {
            inactivityTimer.fireDate = Date(timeIntervalSinceNow: Self.inactivityTimeout)
        } else {
            inactivityTimer = Timer.scheduledTimer(
                withTimeInterval: Self.inactivityTimeout,
                repeats: false
            ) { [weak self] _ in
                MainActor.assumeIsolated {
                    self?.hide()
                }
            }
        }
    }

    private func stopInactivity() {
        inactivityTimer?.invalidate()
        inactivityTimer = nil
        stopMonitoringActivity()
    }

    private func stopMonitoringActivity() {
        guard let activityMonitor else { return }
        NSEvent.removeMonitor(activityMonitor)
        self.activityMonitor = nil
    }
}
