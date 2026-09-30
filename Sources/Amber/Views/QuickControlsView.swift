import SwiftUI
import AmberKit

/// The menu-bar popup.
///
/// Ported from `Views/QuickControlsWindow.axaml`: brightness, colour temperature, the automatic-mode
/// toggle, and the Settings/Quit buttons. The gates match the settings window exactly — the
/// brightness slider needs DDC/CI support, and the manual temperature slider needs automatic mode
/// off *and* a gamma-capable display — so neither offers a control the backend cannot apply.
///
/// The view supplies the popup's rounded material surface, because the borderless panel behind it
/// draws no chrome of its own. Controls use the system appearance, independent of the selected
/// settings palette.
struct QuickControlsView: View {

    @Bindable var viewModel: MainViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            brightnessControls
            colorTemperatureControls

            HStack(spacing: 8) {
                Button("Settings") {
                    viewModel.showSettings()
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
                .tint(Color(nsColor: .systemGray))

                Spacer()

                Button("Quit") {
                    viewModel.quit()
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .tint(Color(nsColor: .systemGray))
            }
        }
        .padding(16)
        .frame(width: 320, alignment: .leading)
        .background(VisualEffectBackground(cornerRadius: Self.cornerRadius))
        .overlay {
            RoundedRectangle(cornerRadius: Self.cornerRadius, style: .continuous)
                .strokeBorder(Color.primary.opacity(Self.borderOpacity), lineWidth: 1)
        }
    }

    /// Corner radius shared by the material surface and its outline, so the hairline cannot
    /// drift away from the shape it is tracing.
    private static let cornerRadius: CGFloat = 12

    /// Opacity of that hairline. Measured off a real `NSPopover` capture: AppKit's own edge lands
    /// around a third of the way to black over the fill in a light appearance. The 10 % this used
    /// to be left no visible pixel at all, so over a coloured desktop the panel had no defined
    /// edge and read as a flat smear rather than a surface.
    private static let borderOpacity: Double = 0.3

    // MARK: - Brightness

    private var brightnessControls: some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack {
                sectionTitle("Brightness")
                Spacer()
                valueLabel("\(viewModel.brightness)%")
            }

            Slider(value: brightnessBinding, in: 1...100)
                .tint(Color(nsColor: .systemGray))
                .disabled(!viewModel.hasBrightnessSupport)
        }
    }

    // MARK: - Colour temperature

    private var colorTemperatureControls: some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack {
                sectionTitle("Color temperature")
                Spacer()
                valueLabel("\(viewModel.colorTemperature) K")
            }

            TemperatureSlider(
                value: colorTemperatureBinding,
                isEnabled: viewModel.manualColorEnabled
            )

            Toggle("Automatic color", isOn: $viewModel.autoColorTemperature)
                .font(.system(size: 12))
                .toggleStyle(.switch)
                .tint(Color(nsColor: .systemGray))
                .controlSize(.small)
        }
    }

    // MARK: - Labels

    private func sectionTitle(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(.primary)
    }

    private func valueLabel(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 12, weight: .semibold))
            .monospacedDigit()
            .foregroundStyle(.primary)
    }

    // MARK: - Bindings

    private var brightnessBinding: Binding<Double> {
        Binding(
            get: { Double(viewModel.brightness) },
            set: { viewModel.brightness = Int($0.rounded()) }
        )
    }

    /// No snapping here: ``TemperatureSlider`` already resolves every position through
    /// ``TemperatureScale``, which snaps to the kelvin step and clamps to the supported range.
    private var colorTemperatureBinding: Binding<Int> {
        Binding(
            get: { viewModel.colorTemperature },
            set: { viewModel.colorTemperature = $0 }
        )
    }
}
