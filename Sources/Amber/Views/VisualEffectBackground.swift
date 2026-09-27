import AppKit
import SwiftUI

/// AppKit's own popover material as a SwiftUI background.
///
/// SwiftUI's `Material` values approximate the system materials but are noticeably flatter than
/// the real thing: `.regularMaterial` is too opaque to read as an Apple popover. `NSVisualEffectView`
/// with `.popover` blending *behind the window* is what AppKit itself puts behind a popover, so it
/// gives the same vibrancy and the same response to light/dark.
///
/// Corner rounding happens on the layer rather than in SwiftUI so the blur is clipped too.
struct VisualEffectBackground: NSViewRepresentable {

    var material: NSVisualEffectView.Material = .popover
    var cornerRadius: CGFloat = 12

    func makeNSView(context: Context) -> NSVisualEffectView {
        let view = NSVisualEffectView()
        view.blendingMode = .behindWindow
        view.state = .active
        view.material = material
        view.wantsLayer = true
        view.layer?.cornerRadius = cornerRadius
        view.layer?.cornerCurve = .continuous
        view.layer?.masksToBounds = true
        return view
    }

    func updateNSView(_ view: NSVisualEffectView, context: Context) {
        view.material = material
        view.layer?.cornerRadius = cornerRadius
    }
}
