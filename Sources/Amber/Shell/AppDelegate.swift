import AppKit
import SwiftUI
import AmberKit

/// The composition root and app lifecycle.
///
/// Ported from `App.axaml.cs`. Composition is deliberately hand-rolled — there is no DI
/// container, matching the original.
///
/// Order matters and mirrors the .NET version:
/// 1. Load settings and apply the palette for the current appearance.
/// 2. Build the native services.
/// 3. Build the scheduler and the shared main view model.
/// 4. Wire callbacks, create the popup and settings window.
/// 5. Apply the persisted state to the hardware and start the scheduler.
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {

    static let shared = AppDelegate()

    private var statusItem: NSStatusItem?
    private var quickControlsPanel: MenuBarPanel?
    private var outsideClickMonitor: Any?
    private var settingsWindow: HidingWindow?

    private var themeService: ThemeService?
    private var schedule: ScheduleService?
    private var colorService: ColorTemperatureService?
    private var brightnessService: AppleSiliconBrightnessService?
    private var loginItemService: LoginItemService?
    private var viewModel: MainViewModel?

    /// The settings window is sized to the sheet once; see ``fitSettingsWindow(toContentHeight:)``.
    private var hasSizedSettingsWindow = false

    private override init() {
        super.init()
    }

    // MARK: - Lifecycle

    func applicationDidFinishLaunching(_ notification: Notification) {
        let store = SettingsStore()
        let installationStore = InstallationStore()
        let installIdentifier = Bundle.main.object(forInfoDictionaryKey: "AmberInstallationID") as? String
            ?? "legacy-or-unpackaged"
        let isFirstLaunchAfterInstall = !installationStore.hasSeen(identifier: installIdentifier)
        let themeService = ThemeService()

        // Order matters: the brightness backend owns the DDC probe, and the display service needs its
        // answer before it can claim any display is brightness-capable.
        let brightnessService = AppleSiliconBrightnessService()
        let displayService = CoreGraphicsDisplayService(
            ddcBrightnessSupported: brightnessService.isSupported
        )
        let colorService = CoreGraphicsColorTemperatureService(displayService: displayService)
        let schedule = ScheduleService(colorService: colorService)
        let loginItemService = SMAppServiceLoginItemService()

        // The popup and the settings window share this one view model, so slider and
        // automatic-mode state stay in sync without a second settings path.
        let viewModel = MainViewModel(
            store: store,
            displayService: displayService,
            brightnessService: brightnessService,
            colorService: colorService,
            schedule: schedule,
            theme: themeService,
            loginItemService: loginItemService
        )
        viewModel.onShowSettings = { [weak self] in
            self?.closeQuickControls()
            self?.showSettingsWindow()
        }
        viewModel.onQuit = {
            NSApp.terminate(nil)
        }

        self.themeService = themeService
        self.brightnessService = brightnessService
        self.colorService = colorService
        self.schedule = schedule
        self.loginItemService = loginItemService
        self.viewModel = viewModel

        themeService.applyForCurrentMode(settings: viewModel.settings)
        configureStatusItem(viewModel: viewModel, themeService: themeService)

        viewModel.applyInitialState()
        viewModel.applyLoginItemState(isFirstLaunchAfterInstall: isFirstLaunchAfterInstall)
        if isFirstLaunchAfterInstall {
            installationStore.markSeen(identifier: installIdentifier)
        }
        schedule.start()
    }

    /// Stops scheduling, restores ColorSync gamma state, and releases native handles.
    /// Mirrors `OnDesktopExit` in the .NET app.
    func applicationWillTerminate(_ notification: Notification) {
        schedule?.stop()
        colorService?.reset()
        brightnessService?.dispose()
        settingsWindow?.allowClose = true
        stopWatchingForOutsideClicks()
    }

    // MARK: - Status item and popup

    private func configureStatusItem(viewModel: MainViewModel, themeService: ThemeService) {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let button = item.button {
            let image = NSImage(
                systemSymbolName: "sun.max.fill",
                accessibilityDescription: "Night Mode Controller"
            )
            image?.isTemplate = true
            button.image = image
            button.target = self
            button.action = #selector(toggleQuickControls)
        }

        // The panel draws no chrome, so the rounded material surface comes from the SwiftUI content.
        // A hosting view is used directly rather than an `NSPopover` because AppKit gives no way to
        // turn off a popover's anchor arrow.
        let hosting = NSHostingView(
            rootView: QuickControlsView(viewModel: viewModel)
        )

        self.statusItem = item
        self.quickControlsPanel = MenuBarPanel(contentView: hosting)
    }

    @objc private func toggleQuickControls() {
        guard let panel = quickControlsPanel, let button = statusItem?.button else { return }

        if panel.isVisible {
            closeQuickControls()
            return
        }

        position(panel, under: button)

        // A non-activating panel takes key status without activating the app, so the popup opens
        // without pulling focus away from whatever the user was doing.
        panel.makeKeyAndOrderFront(nil)
        watchForOutsideClicks()
    }

    /// Places the popup under the status item, right edges aligned, clamped to the screen.
    private func position(_ panel: NSWindow, under button: NSStatusBarButton) {
        guard let buttonWindow = button.window else { return }

        let buttonFrame = buttonWindow.convertToScreen(button.convert(button.bounds, to: nil))
        let size = panel.frame.size

        var origin = NSPoint(
            x: buttonFrame.maxX - size.width,
            y: buttonFrame.minY - Self.quickControlsGap - size.height
        )

        if let visible = (buttonWindow.screen ?? NSScreen.main)?.visibleFrame {
            origin.x = min(max(origin.x, visible.minX + 8), visible.maxX - size.width - 8)
            origin.y = max(origin.y, visible.minY + 8)
        }

        panel.setFrame(NSRect(origin: origin, size: size), display: true)
    }

    /// Gap between the menu bar and the popup. Apple's own menu-bar popups sit almost flush, so this
    /// is deliberately small — a visible gap reads as the popup hanging lower than the system's.
    private static let quickControlsGap: CGFloat = 2

    private func closeQuickControls() {
        stopWatchingForOutsideClicks()
        quickControlsPanel?.orderOut(nil)
    }

    /// Closes the popup on a click outside it, including clicks that land in another application.
    /// Those are never delivered to the app, and a non-activating panel does not resign key when
    /// another app takes focus, so nothing else would notice them.
    private func watchForOutsideClicks() {
        stopWatchingForOutsideClicks()

        outsideClickMonitor = NSEvent.addGlobalMonitorForEvents(
            matching: [.leftMouseDown, .rightMouseDown]
        ) { [weak self] _ in
            // Global monitors are delivered on the main thread.
            MainActor.assumeIsolated {
                self?.closeQuickControls()
            }
        }
    }

    private func stopWatchingForOutsideClicks() {
        guard let outsideClickMonitor else { return }
        NSEvent.removeMonitor(outsideClickMonitor)
        self.outsideClickMonitor = nil
    }

    // MARK: - Settings window

    private func showSettingsWindow() {
        guard let viewModel, let themeService else { return }

        if settingsWindow == nil {
            let window = HidingWindow(
                contentRect: NSRect(
                    x: 0,
                    y: 0,
                    width: SettingsView.minimumWidth,
                    height: Self.settingsMinimumHeight
                ),
                styleMask: [.titled, .closable, .miniaturizable, .resizable],
                backing: .buffered,
                defer: false
            )
            window.title = "Night Mode Controller"
            window.isReleasedWhenClosed = false
            window.minSize = NSSize(width: SettingsView.minimumWidth, height: 320)

            let hosting = NSHostingController(
                rootView: SettingsView(
                    viewModel: viewModel,
                    themeService: themeService,
                    onHeightChange: { [weak self] height in
                        self?.fitSettingsWindow(toContentHeight: height)
                    }
                )
            )
            // Publish the window *before* attaching the controller. Attaching it is what makes the
            // sheet lay out and report its height, and `fitSettingsWindow` needs a window to act
            // on. That first report is the only accurate measurement — the height does not change
            // again afterwards — so dropping it left the window stuck at its 320 pt minimum with a
            // scroll bar, which is exactly what the fit exists to prevent.
            settingsWindow = window

            window.contentViewController = hosting

            // Assigning the controller lets AppKit size the window to the hosting view's first
            // fitting size, which is far shorter than the sheet. Lay the sheet out here so it
            // reports its real height before the window is ever on screen.
            hosting.view.layoutSubtreeIfNeeded()

            window.center()
        }

        settingsWindow?.showWithInactivityTimeout()
    }

    /// Sizes the settings window to the height the sheet actually needs, so it opens without a
    /// scroll bar.
    ///
    /// Applied **once**. Later reports arrive *because* the window changed size — the sheet
    /// measures a little differently at a different viewport height — so following them makes the
    /// window oscillate. Expanding a collapsed section therefore scrolls rather than growing the
    /// window: that is the trade that keeps the launch state stable.
    private func fitSettingsWindow(toContentHeight height: CGFloat) {
        guard !hasSizedSettingsWindow, let window = settingsWindow else { return }
        hasSizedSettingsWindow = true

        let available = max(
            (NSScreen.main?.visibleFrame.height ?? 900) - 80,
            Self.settingsMinimumHeight
        )
        let target = min(
            max(height, Self.settingsMinimumHeight) + Self.settingsHeightSlack,
            available
        )

        window.setContentSize(NSSize(width: SettingsView.minimumWidth, height: target))
    }

    /// Breathing room so the lowest caption is not flush against the window edge.
    private static let settingsHeightSlack: CGFloat = 8
    private static let settingsMinimumHeight: CGFloat = 420
}
