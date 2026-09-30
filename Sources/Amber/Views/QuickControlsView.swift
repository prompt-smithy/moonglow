import SwiftUI
import AmberKit

/// The menu-bar popup.
///
/// # Status: stub
///
/// Port these controls from `Views/QuickControlsWindow.axaml`:
///
/// - Brightness slider, `1...100`, enabled by `viewModel.hasBrightnessSupport`
/// - Colour-temperature slider, `900...6600`, enabled by `viewModel.manualColorEnabled`
/// - "Automatic color" toggle bound to `viewModel.autoColorTemperature`
/// - "Settings" button calling `viewModel.showSettings()`
/// - "Quit" button calling `viewModel.quit()`
///
/// `@ObservedObject` provides two-way bindings into the shared view model.
struct QuickControlsView: View {

    @ObservedObject var viewModel: MainViewModel
    @ObservedObject var themeService: ThemeService

    private var theme: AppTheme { themeService.current }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Amber")
                .font(.headline)
                .foregroundStyle(theme.strongTextColor)

            Text("Swift port in progress. Quick controls are not wired up yet.")
                .font(.caption)
                .foregroundStyle(theme.secondaryTextColor)

            Divider()

            HStack(spacing: 8) {
                Button("Settings") {
                    viewModel.showSettings()
                }
                Spacer()
                Button("Quit") {
                    viewModel.quit()
                }
            }
        }
        .padding(20)
        .frame(width: 320, alignment: .leading)
        .background(theme.windowBackgroundColor)
    }
}
