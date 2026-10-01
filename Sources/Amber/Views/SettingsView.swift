import SwiftUI
import AmberKit

/// The main settings window.
///
/// The schedule is the primary settings surface, followed by startup, displays, and appearance.
///
/// Morning and evening times use ``TimeOfDayPicker``: separate hour, minute, and AM/PM
/// drop-downs drawn from the palette. The day and night targets share a single two-handle bar,
/// ``TemperatureRangeSlider``, instead of the two separate sliders the .NET window used.
struct SettingsView: View {

    @ObservedObject var viewModel: MainViewModel
    @ObservedObject var themeService: ThemeService

    /// Reports the height the whole sheet needs once it has been laid out so the window can fit it.
    var onHeightChange: ((CGFloat) -> Void)?

    /// Fits two 196 pt time fields, their 12 pt gap, card padding, and outer padding.
    static let minimumWidth: CGFloat = 456

    private var theme: AppTheme { themeService.current }

    /// Laid-out content height reported through ``onHeightChange``.
    @State private var contentHeight: CGFloat = 0

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 10) {
                scheduleSection
                startupSection
                displaysSection
            }
            .padding(14)
            .background(heightReader { contentHeight = $0 })
        }
        .frame(minWidth: Self.minimumWidth, alignment: .topLeading)
        .background(theme.windowBackgroundColor)
        .onChange(of: contentHeight) { height in
            guard height > 0 else { return }
            onHeightChange?(height)
        }
    }

    /// Reports a laid-out height. Attached to the scroll view's *content*, not the scroll view,
    /// which is greedy and would only report the window's height back.
    private func heightReader(_ assign: @escaping (CGFloat) -> Void) -> some View {
        GeometryReader { geometry in
            Color.clear
                .onAppear { assign(geometry.size.height) }
                .onChange(of: geometry.size.height) { height in assign(height) }
        }
    }

    // MARK: - Displays

    private var displaysSection: some View {
        card {
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    sectionTitle("Displays")
                    Spacer()
                    Button("Refresh") { viewModel.refreshDisplays() }
                        .controlSize(.small)
                }

                ForEach(viewModel.displays) { display in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(display.name)
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(theme.primaryTextColor)
                        caption(display.capabilitySummary)
                    }
                }

                if viewModel.hasNoDisplays {
                    caption("No active displays were reported.")
                }
            }
        }
    }

    // MARK: - Colour temperature and schedule

    private var scheduleSection: some View {
        card {
            VStack(alignment: .leading, spacing: 10) {
                Toggle("Automatic (time based)", isOn: $viewModel.autoColorTemperature)
                    .font(.system(size: 13))
                    .foregroundStyle(theme.primaryTextColor)
                    .toggleStyle(.switch)
                    .tint(Color(nsColor: .systemGray))

                temperatureRangeControl

                scheduleGroup(
                    title: "Morning transition (warm → cool)",
                    from: morningStartBinding,
                    to: morningEndBinding
                )
                scheduleGroup(
                    title: "Evening transition (cool → warm)",
                    from: eveningStartBinding,
                    to: eveningEndBinding
                )
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
                caption("WARM \(viewModel.nightColorTemp) K")
                Spacer()
                caption("COOL \(viewModel.dayColorTemp) K")
            }
        }
    }

    private func scheduleGroup(
        title: String,
        from: Binding<TimeOfDay>,
        to: Binding<TimeOfDay>
    ) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(title)
                .font(.system(size: 13, weight: .bold))
                .foregroundStyle(theme.secondaryTextColor)

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
                    .tint(Color(nsColor: .systemGray))

                caption(viewModel.launchAtLoginStatus)

                if viewModel.loginItemRequiresApproval {
                    Button("Open Login Items…") { viewModel.openLoginItemsSettings() }
                        .controlSize(.small)
                }
            }
        }
    }

    // MARK: - Layout helpers

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
