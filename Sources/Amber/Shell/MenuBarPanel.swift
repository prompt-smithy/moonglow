import AppKit

/// The status item's popup: a borderless, non-activating panel.
///
/// `NSPopover` cannot be used for this because it always draws an anchor arrow and AppKit exposes
/// no way to suppress it. Apple's own menu-bar popups — Control Center, the input-source list —
/// are borderless panels like this one: no arrow, rounded corners supplied by the content, and no
/// app activation, so opening the popup leaves the keyboard focus where the user had it.
///
/// The panel draws no chrome of its own. Its content view supplies the rounded material surface.
final class MenuBarPanel: NSPanel {

    init(contentView: NSView) {
        super.init(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )

        isFloatingPanel = true
        level = .statusBar
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        isMovable = false
        isMovableByWindowBackground = false
        animationBehavior = .utilityWindow
        isReleasedWhenClosed = false

        self.contentView = contentView

        // The panel must be sized to the content, not the other way round: a window created with a
        // zero content rect keeps that size, and assigning `contentView` resizes the *view* down to
        // match it. Without this the popup is a zero-sized window that never appears.
        setContentSize(contentView.fittingSize)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("MenuBarPanel is created in code only")
    }

    /// Borderless panels are not key by default. The popup needs key status for its controls, and a
    /// *non-activating* panel can hold it without activating the app.
    override var canBecomeKey: Bool { true }

    override var canBecomeMain: Bool { false }
}
