import AppKit
import CoreGraphics

/// Entry point for the menu-bar app.
///
/// `NSApplication.delegate` is a *weak* reference, so the delegate is held statically by
/// ``AppDelegate/shared`` or it would be deallocated immediately.
///
/// Replaces `Program.cs`: the activation policy is set to `.accessory` so Moonglow
/// has no Dock icon and no app-switcher entry, matching `ShowInDock = false` in the .NET app.
@main
enum MoonglowApp {
    @MainActor
    static func main() {
        _ = signal(SIGTERM, restoreGammaAndExit)
        _ = signal(SIGINT, restoreGammaAndExit)
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)
        app.delegate = AppDelegate.shared
        app.run()
    }
}

/// Restores ColorSync gamma state, then exits with the conventional 128 + signal code.
///
/// A signal handler may only call async-signal-safe functions, which this is not — but
/// the alternative is leaving the display tinted on every SIGTERM/SIGINT, and the exit
/// marker covers the cases where even this cannot run (SIGKILL, crash).
private func restoreGammaAndExit(_ signal: Int32) {
    CGDisplayRestoreColorSyncSettings()
    _exit(128 + signal)
}
