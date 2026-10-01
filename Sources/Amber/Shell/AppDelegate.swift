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
final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {

    static let shared = AppDelegate()

    private var statusItem: NSStatusItem?
    private var quickControlsMenu: NSMenu?
    private var brightnessMenuRow: MenuSliderRow?
    private var temperatureMenuRow: MenuTemperatureSliderRow?
    private var automaticMenuRow: MenuToggleRow?
    private var menuStatusItem: NSMenuItem?
    private var settingsWindow: HidingWindow?

    private var themeService: ThemeService?
    private var schedule: ScheduleService?
    private var colorService: ColorTemperatureService?
    private var brightnessService: BrightnessFallbackService?
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
        let hardwareBrightnessService: BrightnessService
        #if arch(arm64)
        hardwareBrightnessService = AppleSiliconBrightnessService()
        #else
        hardwareBrightnessService = IntelDDCBrightnessService(displayID: CGMainDisplayID())
        #endif
        let displayService = CoreGraphicsDisplayService(
            ddcBrightnessSupported: hardwareBrightnessService.isSupported
        )
        let colorService = CoreGraphicsColorTemperatureService(displayService: displayService)
        let brightnessService = BrightnessFallbackService(
            hardware: hardwareBrightnessService,
            software: CoreGraphicsBrightnessService(colorService: colorService)
        )
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
            loginItemService: loginItemService
        )
        viewModel.onShowSettings = { [weak self] in
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

        configureStatusItem(viewModel: viewModel)

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
    }

    // MARK: - Status item and popup

    private func configureStatusItem(viewModel: MainViewModel) {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let button = item.button {
            let image = NSImage(
                systemSymbolName: "sun.max.fill",
                accessibilityDescription: "Night Mode Controller"
            )
            image?.isTemplate = true
            button.image = image
        }

        let menu = NSMenu()
        menu.autoenablesItems = false
        menu.delegate = self

        let titleItem = NSMenuItem(title: "Night Mode Controller", action: nil, keyEquivalent: "")
        titleItem.isEnabled = false
        menu.addItem(titleItem)
        menu.addItem(.separator())

        let brightnessRow = MenuSliderRow(
            title: "Brightness",
            value: viewModel.brightness,
            minimum: 1,
            maximum: 100,
            target: self,
            action: #selector(brightnessChanged(_:))
        )
        let brightnessItem = NSMenuItem()
        brightnessItem.view = brightnessRow
        menu.addItem(brightnessItem)
        self.brightnessMenuRow = brightnessRow

        let temperatureRow = MenuTemperatureSliderRow(viewModel: viewModel)
        let temperatureItem = NSMenuItem()
        temperatureItem.view = temperatureRow
        menu.addItem(temperatureItem)
        self.temperatureMenuRow = temperatureRow

        let automaticRow = MenuToggleRow(
            title: "Automatic (time based)",
            isOn: viewModel.autoColorTemperature,
            target: self,
            action: #selector(automaticColorTemperatureChanged(_:))
        )
        let automaticItem = NSMenuItem()
        automaticItem.view = automaticRow
        menu.addItem(automaticItem)
        self.automaticMenuRow = automaticRow

        let statusItem = NSMenuItem(title: "", action: nil, keyEquivalent: "")
        statusItem.isEnabled = false
        menu.addItem(.separator())
        menu.addItem(statusItem)
        self.menuStatusItem = statusItem

        menu.addItem(.separator())
        menu.addItem(actionMenuItem(title: "Refresh Displays", action: #selector(refreshDisplays)))
        menu.addItem(actionMenuItem(title: "Settings…", action: #selector(showSettingsFromMenu)))
        menu.addItem(actionMenuItem(title: "Quit", action: #selector(quitFromMenu)))

        self.statusItem = item
        self.quickControlsMenu = menu
        item.menu = menu
        updateQuickControlsMenu(viewModel: viewModel)
    }

    private func actionMenuItem(title: String, action: Selector) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: "")
        item.target = self
        item.view = MenuActionRow(title: title)
        return item
    }

    @objc private func brightnessChanged(_ sender: NSSlider) {
        viewModel?.brightness = sender.integerValue
        if let viewModel { updateQuickControlsMenu(viewModel: viewModel) }
    }

    @objc private func automaticColorTemperatureChanged(_ sender: NSButton) {
        guard let viewModel else { return }
        viewModel.autoColorTemperature = sender.state == .on
        updateQuickControlsMenu(viewModel: viewModel)
    }

    @objc private func refreshDisplays() {
        viewModel?.refreshDisplays()
        if let viewModel { updateQuickControlsMenu(viewModel: viewModel) }
    }

    @objc private func showSettingsFromMenu() {
        viewModel?.showSettings()
    }

    @objc private func quitFromMenu() {
        viewModel?.quit()
    }

    private func updateQuickControlsMenu(viewModel: MainViewModel) {
        brightnessMenuRow?.update(value: viewModel.brightness)
        brightnessMenuRow?.slider.isEnabled = viewModel.hasBrightnessSupport
        automaticMenuRow?.button.state = viewModel.autoColorTemperature ? .on : .off
        menuStatusItem?.title = viewModel.hasNoDisplays
            ? "No active displays found"
            : "\(viewModel.displays.count) display(s) available"
    }

    func menuWillOpen(_ menu: NSMenu) {
        if let viewModel { updateQuickControlsMenu(viewModel: viewModel) }
    }

    func menu(_ menu: NSMenu, willHighlight item: NSMenuItem?) {
        for menuItem in menu.items {
            (menuItem.view as? MenuHoverableRow)?.isMenuHighlighted = menuItem === item
        }
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
