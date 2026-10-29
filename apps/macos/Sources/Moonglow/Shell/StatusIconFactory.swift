import AppKit
import MoonglowKit

/// The menu-bar tray image: a raster of the SVG logo master, in every appearance.
///
/// Master artwork is `resources/Moonglow-option-e-amber-noring.svg` at the repository
/// root; `Resources/TrayIcon.png` is its 1024 px render. macOS cannot rasterize SVG at
/// runtime, so the PNG is bundled and this factory only scales it to status-item size.
/// If the resource is ever missing, the factory falls back to the drawn crescent below.
enum StatusIconFactory {

    /// The logo at `size` points, ready for a status-bar button.
    static func trayImage(size: CGFloat = 18) -> NSImage {
        if let url = Bundle.module.url(forResource: "TrayIcon", withExtension: "png"),
           let image = NSImage(contentsOf: url)
        {
            image.size = NSSize(width: size, height: size)
            image.isTemplate = false
            image.accessibilityDescription = "Moonglow"
            return image
        }
        return fallbackTrayImage(size: size)
    }

    /// The old two-tone crescent, drawn from `CrescentGeometry`. Resource-missing
    /// fallback only; the shipped icon is the bundled logo above.
    private static func fallbackTrayImage(size: CGFloat) -> NSImage {
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
