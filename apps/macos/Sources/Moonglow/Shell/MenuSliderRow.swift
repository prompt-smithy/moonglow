import AppKit
import SwiftUI
import MoonglowKit

class MenuHoverableRow: NSView {

    var isMenuHighlighted = false {
        didSet { needsDisplay = true }
    }

    override func draw(_ dirtyRect: NSRect) {
        guard isMenuHighlighted else { return }
        NSColor.systemGray.withAlphaComponent(0.18).setFill()
        NSBezierPath(
            roundedRect: bounds.insetBy(dx: 4, dy: 1),
            xRadius: 5,
            yRadius: 5
        ).fill()
    }
}

final class MenuActionRow: MenuHoverableRow {

    init(title: String) {
        super.init(frame: NSRect(x: 0, y: 0, width: 252, height: 28))

        let label = NSTextField(labelWithString: title)
        label.font = .menuFont(ofSize: NSFont.systemFontSize)
        label.textColor = .labelColor
        label.translatesAutoresizingMaskIntoConstraints = false
        addSubview(label)

        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            label.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -8),
            label.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("MenuActionRow is created in code only")
    }

    override func mouseUp(with event: NSEvent) {
        guard let item = enclosingMenuItem, let action = item.action else { return }
        let target = item.target
        item.menu?.cancelTracking()
        NSApp.sendAction(action, to: target, from: item)
    }
}

/// The brightness row hosts the shared triangle slider inside the status-item menu.
final class MenuBrightnessSliderRow: MenuHoverableRow {

    init(viewModel: MainViewModel) {
        super.init(frame: NSRect(x: 0, y: 0, width: 252, height: 52))

        let hostingView = NSHostingView(rootView: MenuBrightnessSliderContent(viewModel: viewModel))
        hostingView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(hostingView)

        NSLayoutConstraint.activate([
            hostingView.leadingAnchor.constraint(equalTo: leadingAnchor),
            hostingView.trailingAnchor.constraint(equalTo: trailingAnchor),
            hostingView.topAnchor.constraint(equalTo: topAnchor),
            hostingView.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("MenuBrightnessSliderRow is created in code only")
    }
}

private struct MenuBrightnessSliderContent: View {

    @ObservedObject var viewModel: MainViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                Text("Brightness")
                Spacer()
                Text("\(viewModel.brightness)%")
                    .monospacedDigit()
            }
            .font(.system(size: NSFont.systemFontSize))
            .foregroundStyle(.primary)

            TriangleSlider(
                value: $viewModel.brightness,
                isEnabled: viewModel.hasBrightnessSupport,
                label: "Brightness",
                valueDescription: { "\($0) percent" }
            )
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 4)
    }
}

/// The colour-temperature row hosts the shared gradient slider inside the status-item menu.
final class MenuTemperatureSliderRow: MenuHoverableRow {

    init(viewModel: MainViewModel) {
        // Taller than the native 48 pt rows: the drawn triangle head needs 24 pt of
        // vertical room under the title.
        super.init(frame: NSRect(x: 0, y: 0, width: 252, height: 52))

        let hostingView = NSHostingView(rootView: MenuTemperatureSliderContent(viewModel: viewModel))
        hostingView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(hostingView)

        NSLayoutConstraint.activate([
            hostingView.leadingAnchor.constraint(equalTo: leadingAnchor),
            hostingView.trailingAnchor.constraint(equalTo: trailingAnchor),
            hostingView.topAnchor.constraint(equalTo: topAnchor),
            hostingView.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("MenuTemperatureSliderRow is created in code only")
    }
}

private struct MenuTemperatureSliderContent: View {

    @ObservedObject var viewModel: MainViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                Text("Color temperature")
                Spacer()
                Text("\(viewModel.colorTemperature.formatted()) K")
                    .monospacedDigit()
            }
            .font(.system(size: NSFont.systemFontSize))
            .foregroundStyle(.primary)

            TemperatureSlider(
                value: $viewModel.colorTemperature,
                isEnabled: viewModel.manualColorEnabled
            )
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 4)
    }
}

/// An interactive menu checkbox that doesn't dismiss the status item's menu when toggled.
final class MenuToggleRow: MenuHoverableRow {

    let button: NSButton

    init(title: String, isOn: Bool, target: AnyObject, action: Selector) {
        button = NSButton(checkboxWithTitle: title, target: target, action: action)
        super.init(frame: NSRect(x: 0, y: 0, width: 252, height: 28))

        button.controlSize = .small
        button.font = .menuFont(ofSize: NSFont.systemFontSize)
        button.contentTintColor = .systemGray
        button.state = isOn ? .on : .off
        button.setAccessibilityLabel(title)
        button.translatesAutoresizingMaskIntoConstraints = false
        addSubview(button)

        NSLayoutConstraint.activate([
            button.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            button.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -8),
            button.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("MenuToggleRow is created in code only")
    }
}
