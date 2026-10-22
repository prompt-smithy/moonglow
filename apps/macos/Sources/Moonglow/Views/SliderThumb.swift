import SwiftUI

/// The handle shared by every drawn slider: an upward-pointing triangle.
///
/// Ported from the Windows build, where the head is a `Path` with
/// `Data="M 8,0 L 16,15 L 0,15 Z"` in a 16 pt `Thumb` and a `Margin="0,-3,0,0"` lift.
/// `Stretch="Uniform"` centres the 15 pt path in the 16 pt box, so the head sits 3 pt
/// above the track centre: the apex rises 10.5 pt and the base drops 4.5 pt from it.
struct SliderThumb: View {

    var fill: Color
    var border: Color

    static let width: CGFloat = 16
    static let height: CGFloat = 15

    /// How far above the track centre the head is centred.
    static let verticalOffset: CGFloat = -3

    /// Vertical room a slider must reserve for the head: the apex reaches 10.5 pt above the
    /// track centre, so a symmetric box needs 21 pt. 24 pt matches the .NET `Thumb` height
    /// and leaves a little air above the apex.
    static let requiredHeight: CGFloat = 24

    var body: some View {
        Triangle()
            .fill(fill)
            .overlay {
                Triangle()
                    .stroke(
                        border,
                        style: StrokeStyle(lineWidth: 1.5, lineJoin: .round)
                    )
            }
            .frame(width: Self.width, height: Self.height)
            .offset(y: Self.verticalOffset)
            .shadow(color: .black.opacity(0.18), radius: 1.5, y: 0.5)
    }
}

/// An isosceles triangle with its apex at the top centre of its rect.
private struct Triangle: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        path.move(to: CGPoint(x: rect.midX, y: rect.minY))
        path.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY))
        path.addLine(to: CGPoint(x: rect.minX, y: rect.maxY))
        path.closeSubpath()
        return path
    }
}
