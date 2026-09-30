import AppKit

/// A compact native slider embedded in the status item's standard menu.
final class MenuSliderRow: NSView {

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

/// An interactive menu checkbox that doesn't dismiss the status item's menu when toggled.
final class MenuToggleRow: NSView {

    let button: NSButton

    init(title: String, isOn: Bool, target: AnyObject, action: Selector) {
        button = NSButton(checkboxWithTitle: title, target: target, action: action)
        super.init(frame: NSRect(x: 0, y: 0, width: 252, height: 28))

        button.controlSize = .small
        button.font = .menuFont(ofSize: NSFont.systemFontSize)
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