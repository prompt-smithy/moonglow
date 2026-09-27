import SwiftUI
import AmberKit

/// The main settings window.
///
/// Ported from `Views/MainWindow.axaml`. Sections and gating follow the original: the
/// brightness slider is gated on `hasBrightnessSupport`, the manual colour-temperature
/// slider on `manualColorEnabled`, and the schedule and appearance controls stay available
/// even when no display can be driven — the same trade-off the .NET window made, so a
/// preference can be set before hardware is attached.
///
/// Morning and evening times use ``TimeOfDayPicker``: separate hour, minute, and AM/PM
/// drop-downs drawn from the palette. The day and night targets share a single two-handle bar,
/// ``TemperatureRangeSlider``, instead of the two separate sliders the .NET window used.
struct SettingsView: View {

    @Bindable var viewModel: MainViewModel
    @Bindable var themeService: ThemeService

    /// Reports the height the whole sheet needs once it has been laid out (and again whenever a
    /// section is expanded or collapsed), so the window can be sized to fit it and the sheet
    /// never opens with a scroll bar.
    var onHeightChange: ((CGFloat) -> Void)?

    /// The sheet's minimum width. The schedule rows are the binding constraint: two 196 pt time
    /// fields plus the FROM/TO label need 456 pt before the surrounding padding, so anything
    /// narrower squeezes the time selector.
    static let minimumWidth: CGFloat = 480

    private var theme: AppTheme { themeService.current }

    /// Displays and Appearance start collapsed: they are reference material rather than
    /// everyday controls, and collapsing them keeps the sheet short.
    @State private var isDisplaysExpanded = false
    @State private var isAppearanceExpanded = false

    /// Laid-out heights, summed and reported through ``onHeightChange``.
    @State private var headerHeight: CGFloat = 0
    @State private var contentHeight: CGFloat = 0

    var body: some View {
        VStack(spacing: 0) {
            header
                .background(heightReader { headerHeight = $0 })

            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    displaysSection
                    brightnessSection
                    colorTemperatureSection
                    appearanceSection
                    startupSection
                }
                .padding(14)
                .background(heightReader { contentHeight = $0 })
            }
        }
        .frame(minWidth: Self.minimumWidth, alignment: .topLeading)
        .background(theme.windowBackgroundColor)
        .onChange(of: headerHeight + contentHeight) { _, total in
            guard total > 0 else { return }
            onHeightChange?(total)
        }
    }

    /// Reports a laid-out height. Attached to the scroll view's *content*, not the scroll view,
    /// which is greedy and would only report the window's height back.
    private func heightReader(_ assign: @escaping (CGFloat) -> Void) -> some View {
        GeometryReader { geometry in
            Color.clear
                .onAppear { assign(geometry.size.height) }
                .onChange(of: geometry.size.height) { _, height in assign(height) }
        }
    }

    // MARK: - Header

    /// Just the app name — the sheet already has a window title, so a subtitle and a platform
    /// badge only cost vertical space.
    private var header: some View {
        Text("Amber")
            .font(.system(size: 15, weight: .bold))
            .foregroundStyle(theme.onAccentTextColor)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 14)
            .padding(.vertical, 7)
            .background(theme.accentColor)
    }

    // MARK: - Displays

    private var displaysSection: some View {
        collapsibleCard(title: "Displays", isExpanded: $isDisplaysExpanded) {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Spacer()
                    Button("Refresh") { viewModel.refreshDisplays() }
                        .controlSize(.small)
                }

                ForEach(viewModel.displays) { display in
                    HStack(spacing: 10) {
                        Circle()
                            .fill(theme.accentColor)
                            .frame(width: 7, height: 7)

                        VStack(alignment: .leading, spacing: 2) {
                            Text(display.name)
                                .font(.system(size: 13, weight: .semibold))
                                .foregroundStyle(theme.primaryTextColor)
                            caption(display.capabilitySummary)
                        }

                        Spacer()

                        caption(display.identifier)
                    }
                }

                if viewModel.hasNoDisplays {
                    caption("No active displays were reported.")
                }
            }
        }
    }

    // MARK: - Brightness

    private var brightnessSection: some View {
        card {
            VStack(alignment: .leading, spacing: 7) {
                HStack {
                    sectionTitle("Brightness")
                    Spacer()
                    valueLabel("\(viewModel.brightness)%")
                }

                Slider(value: brightnessBinding, in: 1...100)
                    .tint(theme.controlAccentColor)
                    .disabled(!viewModel.hasBrightnessSupport)

                caption(viewModel.brightnessStatus)
            }
        }
    }

    // MARK: - Colour temperature and schedule

    private var colorTemperatureSection: some View {
        card {
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    sectionTitle("Color temperature")
                    Spacer()
                    valueLabel("\(viewModel.colorTemperature) K")
                }

                Slider(value: colorTemperatureBinding, in: 900...6600)
                    .tint(theme.controlAccentColor)
                    .disabled(!viewModel.manualColorEnabled)

                Toggle("Automatic (time based)", isOn: $viewModel.autoColorTemperature)
                    .font(.system(size: 13))
                    .foregroundStyle(theme.primaryTextColor)
                    .toggleStyle(.switch)
                    .tint(theme.controlAccentColor)

                temperatureRangeControl

                scheduleGroup(
                    title: "MORNING",
                    from: morningStartBinding,
                    to: morningEndBinding
                )
                scheduleGroup(
                    title: "EVENING",
                    from: eveningStartBinding,
                    to: eveningEndBinding
                )

                caption(viewModel.colorStatus)
            }
        }
    }

    /// One bar with a handle at each end: the warmer night target and the cooler day target.
    /// Replaces the two separate target sliders from the .NET window.
    private var temperatureRangeControl: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                caption("COLOR TEMPERATURE RANGE")
                Spacer()
                valueLabel(temperatureRangeBinding.wrappedValue.displayText)
            }

            TemperatureRangeSlider(range: temperatureRangeBinding, theme: theme)

            HStack {
                caption("NIGHT \(viewModel.nightColorTemp) K")
                Spacer()
                caption("DAY \(viewModel.dayColorTemp) K")
            }
        }
    }

    private func scheduleGroup(
        title: String,
        from: Binding<TimeOfDay>,
        to: Binding<TimeOfDay>
    ) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            caption(title)

            HStack(alignment: .top, spacing: 12) {
                timeField(label: "FROM", time: from)
                timeField(label: "TO", time: to)
            }
        }
    }

    private func timeField(label: String, time: Binding<TimeOfDay>) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            caption(label)
            TimeOfDayPicker(time: time, theme: theme)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    // MARK: - Startup

    private var startupSection: some View {
        card {
            VStack(alignment: .leading, spacing: 8) {
                sectionTitle("Startup")

                Toggle("Launch at login", isOn: $viewModel.launchAtLogin)
                    .font(.system(size: 13))
                    .foregroundStyle(theme.primaryTextColor)
                    .toggleStyle(.switch)
                    .tint(theme.controlAccentColor)

                caption(viewModel.launchAtLoginStatus)

                if viewModel.loginItemRequiresApproval {
                    Button("Open Login Items…") { viewModel.openLoginItemsSettings() }
                        .controlSize(.small)
                }
            }
        }
    }

    // MARK: - Appearance

    private var appearanceSection: some View {
        collapsibleCard(title: "Appearance", isExpanded: $isAppearanceExpanded) {
            HStack(alignment: .top, spacing: 12) {
                themeColumn(
                    title: "LIGHT MODE",
                    selection: $viewModel.lightModeTheme,
                    names: viewModel.lightThemes
                )
                themeColumn(
                    title: "DARK MODE",
                    selection: $viewModel.darkModeTheme,
                    names: viewModel.darkThemes
                )
            }
        }
    }

    private func themeColumn(
        title: String,
        selection: Binding<String>,
        names: [String]
    ) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            caption(title)

            ThemedDropDown(
                options: names,
                selection: selection,
                theme: theme,
                accessibilityName: title,
                title: { $0 },
                listWidth: 200
            )
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    // MARK: - Layout helpers

    /// A card whose body stays hidden until the title row is clicked. Used for the sections
    /// that are reference material, so the sheet opens short.
    private func collapsibleCard<Content: View>(
        title: String,
        isExpanded: Binding<Bool>,
        @ViewBuilder content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: isExpanded.wrappedValue ? 8 : 0) {
            Button {
                withAnimation(.easeInOut(duration: 0.12)) {
                    isExpanded.wrappedValue.toggle()
                }
            } label: {
                HStack(spacing: 6) {
                    sectionTitle(title)

                    Spacer(minLength: 4)

                    Image(systemName: "chevron.right")
                        .font(.system(size: 9, weight: .semibold))
                        .foregroundStyle(theme.secondaryTextColor)
                        .rotationEffect(.degrees(isExpanded.wrappedValue ? 90 : 0))
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(title)
            .accessibilityHint(
                isExpanded.wrappedValue ? "Collapses the section" : "Expands the section"
            )

            if isExpanded.wrappedValue {
                content()
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            theme.cardBackgroundColor,
            in: RoundedRectangle(cornerRadius: 8, style: .continuous)
        )
        .overlay {
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .strokeBorder(theme.borderColor, lineWidth: 1)
        }
    }

    private func card<Content: View>(@ViewBuilder content: () -> Content) -> some View {
        content()
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                theme.cardBackgroundColor,
                in: RoundedRectangle(cornerRadius: 8, style: .continuous)
            )
            .overlay {
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .strokeBorder(theme.borderColor, lineWidth: 1)
            }
    }

    private func sectionTitle(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(theme.strongTextColor)
    }

    private func valueLabel(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 13, weight: .semibold))
            .monospacedDigit()
            .foregroundStyle(theme.strongTextColor)
    }

    private func caption(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 11))
            .foregroundStyle(theme.secondaryTextColor)
    }

    // MARK: - Bindings

    private var brightnessBinding: Binding<Double> {
        Binding(
            get: { Double(viewModel.brightness) },
            set: { viewModel.brightness = Int($0.rounded()) }
        )
    }

    /// Snapping happens here rather than through the slider's `step:`, because SwiftUI draws a row
    /// of tick marks along the track of any stepped slider -- AppKit sliders have no such marks.
    private var colorTemperatureBinding: Binding<Double> {
        Binding(
            get: { Double(viewModel.colorTemperature) },
            set: { viewModel.colorTemperature = TemperatureRange.snap(Int($0.rounded())) }
        )
    }

    private var temperatureRangeBinding: Binding<TemperatureRange> {
        Binding(
            get: { TemperatureRange(settings: viewModel.settings) },
            set: { viewModel.setTemperatureRange($0) }
        )
    }

    private var morningStartBinding: Binding<TimeOfDay> {
        Binding(
            get: { viewModel.morningStart ?? .midnight },
            set: { viewModel.morningStart = $0 }
        )
    }

    private var morningEndBinding: Binding<TimeOfDay> {
        Binding(
            get: { viewModel.morningEnd ?? .midnight },
            set: { viewModel.morningEnd = $0 }
        )
    }

    private var eveningStartBinding: Binding<TimeOfDay> {
        Binding(
            get: { viewModel.eveningStart ?? .midnight },
            set: { viewModel.eveningStart = $0 }
        )
    }

    private var eveningEndBinding: Binding<TimeOfDay> {
        Binding(
            get: { viewModel.eveningEnd ?? .midnight },
            set: { viewModel.eveningEnd = $0 }
        )
    }
}
