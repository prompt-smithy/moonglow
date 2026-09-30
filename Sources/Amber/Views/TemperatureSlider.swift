import SwiftUI
import AmberKit

/// A single-handle slider over the kelvin range, drawn on the warm-to-cool ramp.
///
/// The native `Slider` cannot show a gradient track — `.tint` paints one flat colour — so the
/// track and the handle are drawn directly, the same way ``TemperatureRangeSlider`` does for the
/// settings window's two-handle control. Value maths stays in ``TemperatureScale``, so this view
/// only deals in pixels and cannot produce an unsnapped or out-of-range value.
struct TemperatureSlider: View {

    @Binding var value: Int

    /// When false the control is dimmed and inert, which is what the popup needs while automatic
    /// colour temperature is driving the display instead of the user.
    var isEnabled: Bool = true

    private let handleDiameter: CGFloat = 18
    private let trackHeight: CGFloat = 6

    var body: some View {
        GeometryReader { geometry in
            let width = geometry.size.width
            let x = position(of: value, width: width)

            ZStack(alignment: .leading) {
                Capsule()
                    .fill(Color.primary.opacity(0.12))
                    .frame(height: trackHeight)

                // The whole ramp stays visible rather than only the travelled part: the track is
                // what tells the user which way is warmer.
                Capsule()
                    .fill(TemperatureGradient.linear)
                    .frame(height: trackHeight)

                handle(at: x)
            }
            .frame(height: handleDiameter)
            .contentShape(Rectangle())
            .gesture(dragGesture(width: width))
        }
        .frame(height: handleDiameter)
        // Lighter than the usual disabled dimming. Automatic colour is on by default, so this is
        // the state the popup is normally seen in, and dimming harder washed the warm-to-cool ramp
        // out to the point of being unreadable.
        .opacity(isEnabled ? 1 : 0.55)
        .allowsHitTesting(isEnabled)
        .disabled(!isEnabled)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Color temperature")
        .accessibilityValue("\(value) kelvin")
        .accessibilityAdjustableAction { direction in
            switch direction {
            case .increment:
                value = TemperatureRange.snap(value + AppSettings.kelvinStep)
            case .decrement:
                value = TemperatureRange.snap(value - AppSettings.kelvinStep)
            @unknown default:
                break
            }
        }
    }

    // MARK: - Drawing

    private func handle(at x: CGFloat) -> some View {
        Circle()
            .fill(Color(nsColor: .systemGray))
            .overlay {
                Circle().strokeBorder(Color(nsColor: .controlBackgroundColor), lineWidth: 2)
            }
            .frame(width: handleDiameter, height: handleDiameter)
            .shadow(color: .black.opacity(0.18), radius: 1.5, y: 0.5)
            .offset(x: x - (handleDiameter / 2))
    }

    // MARK: - Geometry

    private func position(of value: Int, width: CGFloat) -> CGFloat {
        let usable = max(width - handleDiameter, 1)
        return (handleDiameter / 2) + (CGFloat(TemperatureScale.fraction(of: value)) * usable)
    }

    private func value(atX x: CGFloat, width: CGFloat) -> Int {
        let usable = max(width - handleDiameter, 1)
        return TemperatureScale.value(atFraction: Double((x - (handleDiameter / 2)) / usable))
    }

    // MARK: - Interaction

    private func dragGesture(width: CGFloat) -> some Gesture {
        DragGesture(minimumDistance: 0)
            .onChanged { drag in
                value = value(atX: drag.location.x, width: width)
            }
    }
}
