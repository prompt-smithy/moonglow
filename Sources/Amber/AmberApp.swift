import AppKit

/// Entry point for the menu-bar app.
///
/// `NSApplication.delegate` is a *weak* reference, so the delegate is held statically by
/// ``AppDelegate/shared`` or it would be deallocated immediately.
///
/// Replaces `Program.cs`: the activation policy is set to `.accessory` so Amber has no
/// Dock icon and no app-switcher entry, matching `ShowInDock = false` in the .NET app.
@main
enum AmberApp {
    @MainActor
    static func main() {
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)
        app.delegate = AppDelegate.shared
        app.run()
    }
}
