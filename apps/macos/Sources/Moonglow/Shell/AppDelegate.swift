import AppKit
import CoreGraphics
import SwiftUI
import MoonglowKit

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
    private var temperatureMenuRow: MenuTemperatureSliderRow?
    private var automaticMenuRow: MenuToggleRow?
    private var menuStatusItem: NSMenuItem?
    private var settingsWindow: HidingWindow?

    private var themeService: ThemeService?
    private var schedule: ScheduleService?
    private var colorService: ColorTemperatureService?
    private var brightnessService: BrightnessFallbackService?
    private var loginItemService: LoginItemService?
    private var hotKeyService: CarbonHotKeyService?
    private var viewModel: MainViewModel?
    private var exitMarker: ExitMarker?

    /// Notification registrations for display changes, held so they can be removed on quit.
    private var displayObservers: [(center: NotificationCenter, token: NSObjectProtocol)] = []

    /// The settings window is sized to the sheet once; see ``fitSettingsWindow(toContentHeight:)``.
    private var hasSizedSettingsWindow = false

    private override init() {
        super.init()
    }

    // MARK: - Lifecycle

    @MainActor
    func applicationDidFinishLaunching(_ notification: Notification) {
        AppLog.shared.start()

        // A second copy would fight the first over settings.json and the gamma ramps
        // (auto-start plus a manual launch makes this realistic). Unpackaged builds
        // have no bundle identifier and skip the check, so development can run
        // alongside an installed copy.
        if let bundleIdentifier = Bundle.main.bundleIdentifier,
           NSRunningApplication.runningApplications(withBundleIdentifier: bundleIdentifier).count > 1
        {
            AppLog.shared.log(.lifecycle, "another copy is already running; quitting")
            NSApp.terminate(nil)
            return
        }

        let exitMarker = ExitMarker(
            fileURL: ExitMarker.defaultFileURL(
                // Per copy: an unpackaged run shares the support directory with the
                // packaged app, and each must judge only its own exit.
                instanceIdentifier: Bundle.main.bundleIdentifier ?? "unpackaged"
            )
        )
        self.exitMarker = exitMarker
        if exitMarker.enter() {
            AppLog.shared.log(.lifecycle, "previous run did not exit cleanly; restoring gamma state")
            CGDisplayRestoreColorSyncSettings()
        }

        let store = SettingsStore()
        let installationStore = InstallationStore()
        let installIdentifier = Bundle.main.object(forInfoDictionaryKey: "MoonglowInstallationID") as? String
            ?? "legacy-or-unpackaged"
        let isFirstLaunchAfterInstall = !installationStore.hasSeen(identifier: installIdentifier)
        AppLog.shared.log(.lifecycle, "launched \(Self.versionDescription), install=\(installIdentifier)\(isFirstLaunchAfterInstall ? " (first launch)" : "")")
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
            ddcBrightnessSupported: { hardwareBrightnessService.isSupported }
        )
        let colorService = CoreGraphicsColorTemperatureService(displayService: displayService)
        let brightnessService = BrightnessFallbackService(
            hardware: hardwareBrightnessService,
            software: CoreGraphicsBrightnessService(colorService: colorService)
        )
        let schedule = ScheduleService(colorService: colorService)
        let loginItemService = SMAppServiceLoginItemService()
        let hotKeyService = CarbonHotKeyService()

        // The popup and the settings window share this one view model, so slider and
        // automatic-mode state stay in sync without a second settings path.
        let viewModel = MainViewModel(
            store: store,
            displayService: displayService,
            brightnessService: brightnessService,
            colorService: colorService,
            schedule: schedule,
            loginItemService: loginItemService,
            hotKeyService: hotKeyService
        )
        viewModel.onShowSettings = { [weak self] in
            self?.showSettingsWindow()
        }
        viewModel.onQuit = {
            NSApp.terminate(nil)
        }
        hotKeyService.handlers = [
            .increaseBrightness: { [weak viewModel] in viewModel?.increaseBrightness() },
            .decreaseBrightness: { [weak viewModel] in viewModel?.decreaseBrightness() },
            .increaseTemperature: { [weak viewModel] in viewModel?.increaseColorTemperature() },
            .decreaseTemperature: { [weak viewModel] in viewModel?.decreaseColorTemperature() },
        ]
        // A temperature key needs automatic mode off to stick. The key may arrive while
        // another app is active, so activate first or the alert hides behind it.
        viewModel.confirmAutomaticModeExit = {
            NSApp.activate(ignoringOtherApps: true)
            let alert = NSAlert()
            alert.messageText = "Turn off the automatic schedule?"
            alert.informativeText = "The shortcut sets a fixed colour temperature, which needs automatic mode turned off first."
            alert.addButton(withTitle: "Turn Off")
            alert.addButton(withTitle: "Cancel")
            return alert.runModal() == .alertFirstButtonReturn
        }

        self.themeService = themeService
        self.brightnessService = brightnessService
        self.colorService = colorService
        self.schedule = schedule
        self.loginItemService = loginItemService
        self.hotKeyService = hotKeyService
        self.viewModel = viewModel

        configureStatusItem(viewModel: viewModel)
        observeDisplayChanges(viewModel: viewModel)

        viewModel.applyInitialState()
        viewModel.applyLoginItemState(isFirstLaunchAfterInstall: isFirstLaunchAfterInstall)
        if isFirstLaunchAfterInstall {
            installationStore.markSeen(identifier: installIdentifier)
        }
        schedule.start()
    }

    /// Stops scheduling, restores ColorSync gamma state, and releases native handles.
    /// Mirrors `OnDesktopExit` in the .NET app.
    @MainActor
    func applicationWillTerminate(_ notification: Notification) {
        AppLog.shared.log(.lifecycle, "quitting")
        schedule?.stop()
        viewModel?.flushSettings()
        colorService?.reset()
        brightnessService?.dispose()
        for observer in displayObservers {
            observer.center.removeObserver(observer.token)
        }
        displayObservers = []
        exitMarker?.exitCleanly()
        settingsWindow?.allowClose = true
    }

    // MARK: - Display changes

    /// Re-enumerates and re-applies after sleep and after a display is attached or detached.
    ///
    /// Both events can clear the gamma tables and both change the display set, and neither
    /// raises anything the rest of the app already listens for. Without this the ramp is
    /// only re-asserted when the schedule target happens to move.
    private func observeDisplayChanges(viewModel: MainViewModel) {
        let workspaceCenter = NSWorkspace.shared.notificationCenter
        displayObservers.append((
            center: workspaceCenter,
            token: workspaceCenter.addObserver(
                forName: NSWorkspace.didWakeNotification,
                object: nil,
                queue: .main
            ) { [weak viewModel] _ in
                MainActor.assumeIsolated {
                    viewModel?.refreshDisplays()
                }
            }
        ))

        let defaultCenter = NotificationCenter.default
        displayObservers.append((
            center: defaultCenter,
            token: defaultCenter.addObserver(
                forName: NSApplication.didChangeScreenParametersNotification,
                object: nil,
                queue: .main
            ) { [weak viewModel] _ in
                MainActor.assumeIsolated {
                    viewModel?.refreshDisplays()
                }
            }
        ))
    }

    // MARK: - Status item and popup

    private func configureStatusItem(viewModel: MainViewModel) {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let button = item.button {
            button.image = StatusIconFactory.trayImage()
        }

        let menu = NSMenu()
        menu.autoenablesItems = false
        menu.delegate = self

        let titleItem = NSMenuItem(title: "Moonglow", action: nil, keyEquivalent: "")
        titleItem.isEnabled = false
        menu.addItem(titleItem)
        menu.addItem(.separator())

        let brightnessRow = MenuBrightnessSliderRow(viewModel: viewModel)
        let brightnessItem = NSMenuItem()
        brightnessItem.view = brightnessRow
        menu.addItem(brightnessItem)

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

        let displayCountItem = NSMenuItem(title: "", action: nil, keyEquivalent: "")
        displayCountItem.isEnabled = false
        menu.addItem(.separator())
        menu.addItem(displayCountItem)
        self.menuStatusItem = displayCountItem

        menu.addItem(.separator())
        menu.addItem(actionMenuItem(
            title: "Refresh Displays",
            action: #selector(refreshDisplays),
            keyEquivalent: "r",
            keyEquivalentModifiers: .command
        ))
        menu.addItem(actionMenuItem(
            title: "Settings…",
            action: #selector(showSettingsFromMenu),
            keyEquivalent: ",",
            keyEquivalentModifiers: .command
        ))
        menu.addItem(actionMenuItem(title: "Quit", action: #selector(quitFromMenu)))

        self.statusItem = item
        self.quickControlsMenu = menu
        item.menu = menu
        updateQuickControlsMenu(viewModel: viewModel)
    }

    private func actionMenuItem(
        title: String,
        action: Selector,
        keyEquivalent: String = "",
        keyEquivalentModifiers: NSEvent.ModifierFlags = []
    ) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: keyEquivalent)
        item.keyEquivalentModifierMask = keyEquivalentModifiers
        item.target = self
        item.view = MenuActionRow(title: title)
        return item
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
        automaticMenuRow?.button.state = viewModel.autoColorTemperature ? .on : .off
        menuStatusItem?.title = viewModel.hasNoDisplays
            ? "No active displays found"
            : "\(viewModel.displays.count) display(s) available"
    }

    func menuWillOpen(_ menu: NSMenu) {
        if let viewModel {
            // Opening the popup is the moment before a brightness change: re-probe while
            // hardware is down so a recovered panel is back before the slider moves.
            // When hardware is up this is a no-op probe plus a fast re-enumeration.
            viewModel.refreshDisplays()
            updateQuickControlsMenu(viewModel: viewModel)
        }
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
            window.title = "Moonglow"
            window.titleVisibility = .hidden
            addTitlebarLabel(to: window)
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

    private func addTitlebarLabel(to window: NSWindow) {
        guard
            let zoomButton = window.standardWindowButton(.zoomButton),
            let titlebarView = zoomButton.superview
        else { return }

        let label = NSTextField(labelWithString: "Moonglow")
        label.font = .systemFont(ofSize: 13, weight: .semibold)
        label.alignment = .left
        label.translatesAutoresizingMaskIntoConstraints = false

        titlebarView.addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: zoomButton.trailingAnchor, constant: 6),
            label.trailingAnchor.constraint(lessThanOrEqualTo: titlebarView.trailingAnchor, constant: -8),
            label.centerYAnchor.constraint(equalTo: zoomButton.centerYAnchor)
        ])
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

    /// `v0.2.0 (2) on arm64, macOS …` for the launch log line.
    private static var versionDescription: String {
        let short = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "?"
        let build = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "?"
        #if arch(arm64)
        let arch = "arm64"
        #else
        let arch = "x86_64"
        #endif
        let os = ProcessInfo.processInfo.operatingSystemVersionString
        return "v\(short) (\(build)) on \(arch), \(os)"
    }
}
