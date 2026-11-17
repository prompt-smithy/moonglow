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
        super.init(frame: NSRect(x: 0, y: 0, width: 252, height: 96))

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
                if viewModel.settings.splitBrightnessControls {
                    Text("Split")
                } else {
                    HStack(spacing: 4) {
                        Text("\(viewModel.brightness)%")
                            .monospacedDigit()
                        if viewModel.showsDualBrightnessMidpoint {
                            Text(viewModel.brightness >= 50 ? "• HW" : "• SW")
                                .font(.system(size: 10, weight: .semibold))
                                .foregroundStyle(viewModel.brightness >= 50 ? .primary : .secondary)
                        }
                    }
                }
                Button {
                    viewModel.splitBrightnessControls.toggle()
                } label: {
                    Image(systemName: viewModel.settings.splitBrightnessControls ? "rectangle.compress.vertical" : "chevron.down")
                        .font(.system(size: 10, weight: .semibold))
                        .foregroundStyle(.secondary)
                        .frame(width: 18, height: 18)
                }
                .buttonStyle(.plain)
                .help(viewModel.settings.splitBrightnessControls ? "Use unified brightness control" : "Show hardware and software controls")
            }
            .font(.system(size: NSFont.systemFontSize))
            .foregroundStyle(.primary)

            if viewModel.settings.splitBrightnessControls {
                brightnessSliderRow(
                    title: "Backlight",
                    value: $viewModel.hardwareBrightness,
                    enabled: viewModel.hardwareBrightnessControlEnabled
                )
                brightnessSliderRow(
                    title: "Digital shade",
                    value: $viewModel.softwareBrightness,
                    enabled: viewModel.softwareBrightnessControlEnabled,
                    minimum: viewModel.softwareBrightnessMinimum
                )
            } else {
                brightnessSlider
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 4)
    }

    private var brightnessSlider: some View {
        VStack(spacing: 3) {
            HStack(spacing: 6) {
                TriangleSlider(
                    value: $viewModel.brightness,
                    range: viewModel.brightnessMinimum...100,
                    isEnabled: viewModel.brightnessControlEnabled,
                    midpoint: viewModel.showsDualBrightnessMidpoint ? 50 : nil,
                    label: "Brightness",
                    valueDescription: { "\($0) percent" }
                )

                if viewModel.showsBrightnessRecoveryInfo {
                    Button(action: viewModel.requestBrightnessRecoverySetup) {
                        Image(systemName: "info.circle")
                            .font(.system(size: 13))
                            .foregroundStyle(.secondary)
                            .frame(width: 20, height: 24)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .help("Brightness limits and recovery shortcut")
                    .accessibilityLabel("Learn about software brightness and set a recovery shortcut")
                }
            }

            if viewModel.showsDualBrightnessMidpoint {
                HStack {
                    Text("Software")
                        .font(.system(size: 9, weight: viewModel.brightness < 50 ? .bold : .medium))
                        .foregroundStyle(viewModel.brightness < 50 ? .primary : .secondary)
                    Spacer()
                    Text("50% (Handoff)")
                        .font(.system(size: 8, weight: .semibold))
                        .foregroundStyle(.secondary)
                    Spacer()
                    Text("Hardware")
                        .font(.system(size: 9, weight: viewModel.brightness >= 50 ? .bold : .medium))
                        .foregroundStyle(viewModel.brightness >= 50 ? .primary : .secondary)
                }
                .padding(.horizontal, 9)
            }
        }
    }

    private func brightnessSliderRow(
        title: String,
        value: Binding<Int>,
        enabled: Bool,
        minimum: Int = 1
    ) -> some View {
        HStack(spacing: 6) {
            Text(title)
                .font(.system(size: 10))
                .frame(width: 74, alignment: .leading)
            TriangleSlider(
                value: value,
                range: minimum...100,
                isEnabled: enabled,
                label: title,
                valueDescription: { "\($0) percent" }
            )
            Text("\(value.wrappedValue)%")
                .font(.system(size: 10, design: .monospaced))
                .frame(width: 32, alignment: .trailing)
            if viewModel.showsBrightnessRecoveryInfo {
                Button(action: viewModel.requestBrightnessRecoverySetup) {
                    Image(systemName: "info.circle")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                        .frame(width: 18, height: 20)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help("Brightness capability and recovery shortcut setup")
                .accessibilityLabel("Learn about brightness and set a recovery shortcut")
            }
        }
        .padding(.leading, 2)
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
