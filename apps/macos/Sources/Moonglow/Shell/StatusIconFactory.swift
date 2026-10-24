import AppKit

/// Draws the shared Moonglow crescent as a template image, so the menu bar tints it for the
/// current appearance on its own.
///
/// Geometry mirrors `tools/generate-logo-assets.ps1`, which produces the icons used by the
/// Windows and Linux builds.
enum StatusIconFactory {
    private static let design: Double = 1024
    private static let crescentRadius: Double = 480
    private static let cutRadiusFactor: Double = 1.05
    private static let crescentThickness: Double = 0.6
    /// Positive here, because AppKit draws with the origin in the bottom-left corner; the
    /// shared artwork opens the crescent towards the upper right.
    private static let cutAngleDegrees: Double = 45

    /// The crescent at `size` points, ready for a status-bar button.
    static func trayImage(size: CGFloat = 18) -> NSImage {
        let image = NSImage(size: NSSize(width: size, height: size), flipped: false) { _ in
            NSColor.black.setFill()
            crescentPath(scale: Double(size) / design).fill()
            return true
        }
        image.isTemplate = true
        image.accessibilityDescription = "Moonglow"
        return image
    }

    /// The disc rim on the side away from the cut, then the rim of the cut circle back to where
    /// it started. The outline is shifted so its bounding box is centred, which is what keeps
    /// the mark optically centred.
    private static func crescentPath(scale: Double) -> NSBezierPath {
        let center = design / 2 * scale
        let radius = crescentRadius * scale
        let cutRadius = radius * cutRadiusFactor
        let distance = cutRadius - radius + radius * crescentThickness
        let theta = cutAngleDegrees * .pi / 180

        let along = (distance * distance + radius * radius - cutRadius * cutRadius) / (2 * distance)
        let halfChord = max(0, radius * radius - along * along).squareRoot()
        let cutX = center + cos(theta) * distance
        let cutY = center + sin(theta) * distance

        let beta = atan2(halfChord, along) * 180 / .pi
        let gamma = atan2(halfChord, along - distance) * 180 / .pi

        var points = arcPoints(cx: center, cy: center, radius: radius,
                               startDeg: cutAngleDegrees + beta,
                               endDeg: cutAngleDegrees - beta,
                               midDeg: cutAngleDegrees + 180)
        let inner = arcPoints(cx: cutX, cy: cutY, radius: cutRadius,
                              startDeg: cutAngleDegrees + gamma,
                              endDeg: cutAngleDegrees - gamma,
                              midDeg: cutAngleDegrees + 180)
        // Walk the inner rim back to the start so the outline stays continuous.
        points.append(contentsOf: inner.dropFirst().dropLast())

        let minX = points.map(\.x).min() ?? 0
        let maxX = points.map(\.x).max() ?? 0
        let minY = points.map(\.y).min() ?? 0
        let maxY = points.map(\.y).max() ?? 0
        let shiftX = CGFloat(center - Double(minX + maxX) / 2)
        let shiftY = CGFloat(center - Double(minY + maxY) / 2)

        let path = NSBezierPath()
        guard let start = points.first else { return path }
        path.move(to: NSPoint(x: start.x + shiftX, y: start.y + shiftY))
        for point in points.dropFirst() {
            path.line(to: NSPoint(x: point.x + shiftX, y: point.y + shiftY))
        }
        path.close()
        return path
    }

    /// Samples a circular arc one degree at a time, always travelling in the direction that
    /// passes through `midDeg`.
    private static func arcPoints(cx: Double, cy: Double, radius: Double,
                                  startDeg: Double, endDeg: Double, midDeg: Double) -> [NSPoint] {
        let clockwiseEnd = normalize(endDeg - startDeg)
        let clockwiseMid = normalize(midDeg - startDeg)
        let sweep = clockwiseMid <= clockwiseEnd ? clockwiseEnd : clockwiseEnd - 360

        let steps = max(8, Int(abs(sweep).rounded(.up)))
        return (0...steps).map { index in
            let angle = (startDeg + sweep * Double(index) / Double(steps)) * .pi / 180
            return NSPoint(x: CGFloat(cx + cos(angle) * radius), y: CGFloat(cy + sin(angle) * radius))
        }
    }

    private static func normalize(_ angle: Double) -> Double {
        let wrapped = angle.truncatingRemainder(dividingBy: 360)
        return wrapped < 0 ? wrapped + 360 : wrapped
    }
}
