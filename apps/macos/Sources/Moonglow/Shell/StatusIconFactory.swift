import AppKit
import MoonglowKit

/// Draws the shared Moonglow crescent: a black moon with a white keyline, the same
/// two-tone mark as the packaged icon, in every appearance.
///
/// The outline comes from `CrescentGeometry`, the single definition of the mark for
/// this app; this factory only scales it and paints the two tones.
enum StatusIconFactory {

    /// The crescent at `size` points, ready for a status-bar button.
    static func trayImage(size: CGFloat = 18) -> NSImage { // Keyline mirrors tools/generate-logo-assets.ps1.
        let scale = Double(size) / CrescentGeometry.design
        let lineWidth = CGFloat(2.5 * max(CrescentGeometry.keylineDesign * Double(size) / CrescentGeometry.design, 1.15))
        let path = NSBezierPath()
        let points = CrescentGeometry.outline()
        if let start = points.first {
            path.move(to: NSPoint(x: start.x * scale, y: start.y * scale))
            for point in points.dropFirst() {
                path.line(to: NSPoint(x: point.x * scale, y: point.y * scale))
            }
            path.close()
        }
        let image = NSImage(size: NSSize(width: size, height: size), flipped: false) { _ in
            NSColor.white.setStroke(); path.lineWidth = lineWidth; path.stroke()
            NSColor.black.setFill(); path.fill(); return true
        }
        image.isTemplate = false
        image.accessibilityDescription = "Moonglow"
        return image
    }
}
