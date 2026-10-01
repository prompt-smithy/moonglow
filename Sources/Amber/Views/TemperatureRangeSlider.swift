import SwiftUI
import AmberKit

/// A two-handle slider over the kelvin range.
///
/// Replaces the separate day and night sliders from `MainWindow.axaml`: one bar whose ends
/// are the two schedule targets. macOS has no range slider in SwiftUI, so the track and
/// handles are drawn directly and driven by one drag gesture.
///
/// All value maths — snapping, ordering, and the fraction-to-kelvin conversion — lives in
/// ``TemperatureRange``/``TemperatureScale`` in `AmberKit`, so this view only deals in
/// pixels and cannot produce an invalid range.
struct TemperatureRangeSlider: View {

    @Binding var range: TemperatureRange
    let theme: AppTheme

    /// Which handle the in-flight drag is moving. Resolved once, when the drag starts, so the
    /// handle the user grabbed keeps following the pointer even when the two are pushed
    /// together.
    @State private var draggedHandle: Handle?

    private enum Handle {
        case night
        case day
    }

    private let handleDiameter: CGFloat = 18
    private let trackHeight: CGFloat = 6

    var body: some View {
        GeometryReader { geometry in
            let width = geometry.size.width
            let nightX = position(of: range.night, width: width)
            let dayX = position(of: range.day, width: width)

            ZStack(alignment: .leading) {
                Capsule()
                    .fill(theme.controlBackgroundColor)
                    .frame(height: trackHeight)

                Capsule()
                    .fill(TemperatureGradient.linear)
                    .frame(width: max(dayX - nightX, 0), height: trackHeight)
                    .offset(x: nightX)

                handle(at: nightX)
                handle(at: dayX)
            }
            .frame(height: handleDiameter)
            .contentShape(Rectangle())
            .gesture(dragGesture(width: width))
        }
        .frame(height: handleDiameter)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Color temperature range")
        .accessibilityValue(range.displayText)
    }

    // MARK: - Drawing

    private func handle(at x: CGFloat) -> some View {
        Circle()
            .fill(Color(nsColor: .systemGray))
            .overlay {
                Circle()
                    .strokeBorder(theme.cardBackgroundColor, lineWidth: 2)
            }
            .frame(width: handleDiameter, height: handleDiameter)
            .shadow(color: .black.opacity(0.18), radius: 1.5, y: 0.5)
            .offset(x: x - (handleDiameter / 2))
    }

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
                let target = value(atX: drag.location.x, width: width)

                if draggedHandle == nil {
                    // Grab the nearer handle, so clicking anywhere on the track works.
                    draggedHandle = abs(target - range.night) <= abs(target - range.day)
                        ? .night
                        : .day
                }

                switch draggedHandle {
                case .night:
                    range = range.movingNight(to: target)
                case .day:
                    range = range.movingDay(to: target)
                case nil:
                    break
                }
            }
            .onEnded { _ in
                draggedHandle = nil
            }
    }
}
