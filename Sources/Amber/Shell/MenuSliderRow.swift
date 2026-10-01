import AppKit
import SwiftUI
import AmberKit

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

/// A compact native slider embedded in the status item's standard menu.
final class MenuSliderRow: MenuHoverableRow {

    let slider: NSSlider

    private let title: String
    private let unit: String
    private let valueLabel: NSTextField

    init(
        title: String,
        unit: String = "%",
        value: Int,
        minimum: Int,
        maximum: Int,
        target: AnyObject,
        action: Selector
    ) {
        self.title = title
        self.unit = unit
        self.valueLabel = NSTextField(labelWithString: "")
        self.slider = NSSlider(
            value: Double(value),
            minValue: Double(minimum),
            maxValue: Double(maximum),
            target: target,
            action: action
        )

        super.init(frame: NSRect(x: 0, y: 0, width: 252, height: 48))

        valueLabel.font = .menuFont(ofSize: NSFont.systemFontSize)
        valueLabel.textColor = .labelColor
        slider.controlSize = .small
        slider.trackFillColor = .systemGray
        slider.isContinuous = true
        slider.setAccessibilityLabel(title)

        let stack = NSStackView(views: [valueLabel, slider])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 2
        stack.edgeInsets = NSEdgeInsets(top: 4, left: 12, bottom: 4, right: 12)
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor),
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
            slider.widthAnchor.constraint(equalToConstant: 228),
        ])

        update(value: value)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("MenuSliderRow is created in code only")
    }

    func update(value: Int) {
        slider.integerValue = value
        let formattedValue = unit == " K" ? value.formatted() : "\(value)"
        valueLabel.stringValue = "\(title)  \(formattedValue)\(unit)"
    }
}

/// The colour-temperature row hosts the shared gradient slider inside the status-item menu.
final class MenuTemperatureSliderRow: MenuHoverableRow {

    init(viewModel: MainViewModel) {
        super.init(frame: NSRect(x: 0, y: 0, width: 252, height: 48))

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