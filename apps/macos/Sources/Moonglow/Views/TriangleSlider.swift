import SwiftUI

/// A single-handle slider with the shared triangle head over a plain track.
///
/// The status-item menu's brightness row needs the same head as the temperature sliders but
/// has no gradient to show, and the native `Slider` cannot be restyled, so the track, the
/// filled part and the head are drawn directly here.
struct TriangleSlider: View {

    @Binding var value: Int

    var range: ClosedRange<Int> = 1...100
    var step: Int = 1

    /// When false the control is dimmed and inert, matching ``TemperatureSlider``.
    var isEnabled: Bool = true

    var label: String
    var valueDescription: (Int) -> String

    /// Width the head travels within. Matches the other drawn sliders so every track in the
    /// app starts and ends at the same inset.
    private let travelInset: CGFloat = 18
    private let trackHeight: CGFloat = 6

    var body: some View {
        GeometryReader { geometry in
            let width = geometry.size.width
            let x = position(of: value, width: width)

            ZStack(alignment: .leading) {
                Capsule()
                    .fill(Color.primary.opacity(0.12))
                    .frame(height: trackHeight)

                Capsule()
                    .fill(Color(nsColor: .systemGray))
                    .frame(width: max(x, 0), height: trackHeight)

                SliderThumb(
                    fill: Color(nsColor: .systemGray),
                    border: Color(nsColor: .controlBackgroundColor)
                )
                .offset(x: x - (SliderThumb.width / 2))
            }
            .frame(height: SliderThumb.requiredHeight)
            .contentShape(Rectangle())
            .gesture(dragGesture(width: width))
        }
        .frame(height: SliderThumb.requiredHeight)
        .opacity(isEnabled ? 1 : 0.55)
        .allowsHitTesting(isEnabled)
        .disabled(!isEnabled)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(label)
        .accessibilityValue(valueDescription(value))
        .accessibilityAdjustableAction { direction in
            switch direction {
            case .increment:
                value = clamped(value + step)
            case .decrement:
                value = clamped(value - step)
            @unknown default:
                break
            }
        }
    }

    // MARK: - Geometry

    private func position(of value: Int, width: CGFloat) -> CGFloat {
        let usable = max(width - travelInset, 1)
        let span = max(range.upperBound - range.lowerBound, 1)
        let fraction = Double(value - range.lowerBound) / Double(span)
        return (travelInset / 2) + (CGFloat(min(max(fraction, 0), 1)) * usable)
    }

    private func value(atX x: CGFloat, width: CGFloat) -> Int {
        let usable = max(width - travelInset, 1)
        let fraction = min(max(Double((x - (travelInset / 2)) / usable), 0), 1)
        let raw = Double(range.lowerBound) + (fraction * Double(range.upperBound - range.lowerBound))
        let snapped = (raw / Double(max(step, 1))).rounded() * Double(max(step, 1))
        return clamped(Int(snapped))
    }

    private func clamped(_ value: Int) -> Int {
        min(max(value, range.lowerBound), range.upperBound)
    }

    // MARK: - Interaction

    private func dragGesture(width: CGFloat) -> some Gesture {
        DragGesture(minimumDistance: 0)
            .onChanged { drag in
                value = value(atX: drag.location.x, width: width)
            }
    }
}
