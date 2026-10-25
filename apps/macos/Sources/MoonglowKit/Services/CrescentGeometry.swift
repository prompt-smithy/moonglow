import Foundation

/// Design-space outline of the shared Moonglow crescent.
///
/// Single definition of the mark's geometry for the macOS app. The constants mirror
/// `tools/generate-logo-assets.ps1`, which renders the packaged icons from them; the
/// menu-bar status item draws this same outline at runtime (see `StatusIconFactory`).
///
/// Coordinates are y-up in a 1024-unit design space, so they map directly onto AppKit
/// paths. The outline is centred on its own bounding box and ordered as one continuous
/// loop: the outer rim, then the cut rim walked back to the start.
public enum CrescentGeometry {

    /// Width and height of the design space; every asset scales down from here.
    public static let design: Double = 1024
    /// Radius of the outer rim.
    public static let crescentRadius: Double = 480
    /// Radius of the subtracted cut circle, as a multiple of the outer rim.
    public static let cutRadiusFactor: Double = 1.05
    /// Belly thickness as a fraction of the outer radius.
    public static let crescentThickness: Double = 0.6
    /// The crescent opens towards the upper right. Positive here because the design
    /// space is y-up; the PowerShell master uses −45° in its y-down space for the
    /// same visual opening.
    public static let cutAngleDegrees: Double = 45
    /// Two-tone keyline width, in design units.
    public static let keylineDesign: Double = 24

    public struct Point: Equatable {
        public var x: Double
        public var y: Double
        public init(x: Double, y: Double) {
            self.x = x
            self.y = y
        }
    }

    /// Ordered, bounding-box-centred outline loop.
    public static func outline() -> [Point] {
        var points = outerArc()
        // Walk the inner rim back to the start so the outline stays one continuous,
        // non-self-intersecting loop. Appending it in forward order folds the polygon
        // into a bowtie that fills as a half disc instead of a crescent.
        points.append(contentsOf: innerArc().reversed().dropFirst().dropLast())
        return centred(points)
    }

    /// The disc rim on the side away from the cut, between its two intersections with
    /// the cut circle. Exposed for the ordering test; prefer `outline()`.
    static func outerArc() -> [Point] {
        let frame = Frame()
        return arcPoints(
            cx: frame.center, cy: frame.center, radius: frame.radius,
            startDeg: cutAngleDegrees + frame.beta,
            endDeg: cutAngleDegrees - frame.beta,
            midDeg: cutAngleDegrees + 180
        )
    }

    /// The rim of the cut circle between its two intersections with the disc rim, in
    /// forward order. Exposed for the ordering test; prefer `outline()`.
    static func innerArc() -> [Point] {
        let frame = Frame()
        return arcPoints(
            cx: frame.cutX, cy: frame.cutY, radius: frame.cutRadius,
            startDeg: cutAngleDegrees + frame.gamma,
            endDeg: cutAngleDegrees - frame.gamma,
            midDeg: cutAngleDegrees + 180
        )
    }

    /// Shared circle-intersection frame for both rims.
    private struct Frame {
        let center = design / 2
        let radius = crescentRadius
        let cutRadius = crescentRadius * cutRadiusFactor
        let distance = crescentRadius * cutRadiusFactor - crescentRadius
            + crescentRadius * crescentThickness
        let theta = cutAngleDegrees * .pi / 180

        var cutX: Double { center + cos(theta) * distance }
        var cutY: Double { center + sin(theta) * distance }

        /// Distance from the disc centre to the chord between the intersections.
        var along: Double {
            (distance * distance + radius * radius - cutRadius * cutRadius) / (2 * distance)
        }
        /// Half the chord between the intersections.
        var halfChord: Double { max(0, radius * radius - along * along).squareRoot() }
        /// Half-angle of the outer arc's missing wedge, and of the inner arc's kept span.
        var beta: Double { atan2(halfChord, along) * 180 / .pi }
        var gamma: Double { atan2(halfChord, along - distance) * 180 / .pi }
    }

    /// Samples a circular arc one degree at a time, always travelling in the direction
    /// that passes through `midDeg`.
    private static func arcPoints(
        cx: Double, cy: Double, radius: Double,
        startDeg: Double, endDeg: Double, midDeg: Double
    ) -> [Point] {
        let clockwiseEnd = normalize(endDeg - startDeg)
        let clockwiseMid = normalize(midDeg - startDeg)
        let sweep = clockwiseMid <= clockwiseEnd ? clockwiseEnd : clockwiseEnd - 360

        let steps = max(8, Int(abs(sweep).rounded(.up)))
        return (0...steps).map { index in
            let angle = (startDeg + sweep * Double(index) / Double(steps)) * .pi / 180
            return Point(x: cx + cos(angle) * radius, y: cy + sin(angle) * radius)
        }
    }

    /// Shifts the points so their bounding box is centred in the design space, which
    /// keeps the mark optically centred.
    private static func centred(_ points: [Point]) -> [Point] {
        guard let minX = points.map(\.x).min(), let maxX = points.map(\.x).max(),
              let minY = points.map(\.y).min(), let maxY = points.map(\.y).max()
        else { return points }
        let shiftX = design / 2 - (minX + maxX) / 2
        let shiftY = design / 2 - (minY + maxY) / 2
        return points.map { Point(x: $0.x + shiftX, y: $0.y + shiftY) }
    }

    private static func normalize(_ angle: Double) -> Double {
        let wrapped = angle.truncatingRemainder(dividingBy: 360)
        return wrapped < 0 ? wrapped + 360 : wrapped
    }
}
