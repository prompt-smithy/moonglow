import SwiftUI
import MoonglowKit

/// A drop-down that draws its own field and its own list with the active palette.
///
/// The native `Picker` menu styles both its bezel and its selected row with the *system*
/// accent colour, which shows up as system blue inside the app palette. That colour is not
/// addressable from SwiftUI — `NSMenu` highlighting follows the user's macOS accent setting,
/// and `.tint` does not repaint it — so the control draws its own field and its own scrolling
/// list to stay on-palette.
struct ThemedDropDown<Value: Hashable>: View {

    let options: [Value]
    @Binding var selection: Value
    let theme: AppTheme
    let accessibilityName: String

    /// Title for a value, so callers can format numbers without wrapping them in a type.
    let title: (Value) -> String

    /// Fixed field width, or `nil` to fill the width the parent offers.
    var fieldWidth: CGFloat? = nil
    var listWidth: CGFloat = 180

    @State private var isExpanded = false
    @State private var hovered: Value?

    private let rowHeight: CGFloat = 22
    private let maxVisibleRows = 10

    private var shape: RoundedRectangle {
        RoundedRectangle(cornerRadius: 6, style: .continuous)
    }

    var body: some View {
        Button {
            isExpanded.toggle()
        } label: {
            HStack(spacing: 6) {
                Text(title(selection))
                    .font(.system(size: 13))
                    .foregroundStyle(theme.primaryTextColor)
                    .lineLimit(1)
                    .truncationMode(.tail)

                Spacer(minLength: 2)

                Image(systemName: "chevron.down")
                    .font(.system(size: 8, weight: .semibold))
                    .foregroundStyle(isExpanded ? theme.controlAccentColor : theme.secondaryTextColor)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(shape.fill(theme.controlBackgroundColor))
            .overlay {
                shape.strokeBorder(
                    isExpanded ? theme.controlAccentColor : theme.borderColor,
                    lineWidth: 1
                )
            }
            .contentShape(shape)
        }
        .buttonStyle(.plain)
        .frame(width: fieldWidth)
        .accessibilityLabel(accessibilityName)
        .accessibilityValue(title(selection))
        .popover(isPresented: $isExpanded, arrowEdge: .bottom) {
            list
        }
    }

    // MARK: - List

    private var list: some View {
        ScrollViewReader { proxy in
            ScrollView(.vertical) {
                LazyVStack(spacing: 1) {
                    ForEach(options, id: \.self) { option in
                        row(option)
                    }
                }
                .padding(4)
            }
            .frame(width: listWidth, height: listHeight)
            .onAppear {
                // A turn later: scrolling before the rows are laid out is ignored, which
                // would open a long list at the top instead of at the current value.
                let target = selection
                Task { @MainActor in
                    proxy.scrollTo(target, anchor: .center)
                }
            }
        }
        .background(theme.cardBackgroundColor)
    }

    /// Enough for the options, up to a screen-friendly maximum — the minute list holds sixty.
    private var listHeight: CGFloat {
        (CGFloat(min(options.count, maxVisibleRows)) * rowHeight) + 8
    }

    private func row(_ option: Value) -> some View {
        let isSelected = option == selection

        return Button {
            selection = option
            isExpanded = false
        } label: {
            HStack(spacing: 6) {
                Text(title(option))
                    .font(.system(size: 12, weight: isSelected ? .semibold : .regular))
                    .lineLimit(1)

                Spacer(minLength: 4)

                if isSelected {
                    Image(systemName: "checkmark")
                        .font(.system(size: 9, weight: .bold))
                }
            }
            .foregroundStyle(isSelected ? theme.onAccentTextColor : theme.primaryTextColor)
            .padding(.horizontal, 6)
            .frame(height: rowHeight)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                fill(for: option),
                in: RoundedRectangle(cornerRadius: 4, style: .continuous)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .id(option)
        .onHover { hovering in
            if hovering {
                hovered = option
            } else if hovered == option {
                hovered = nil
            }
        }
        .accessibilityLabel(title(option))
        .accessibilityAddTraits(isSelected ? [.isSelected] : [])
    }

    /// The palette's `accent`/`onAccentText` pair is chosen for contrast, so the selected row
    /// uses that rather than the control accent.
    private func fill(for option: Value) -> Color {
        if option == selection { return theme.accentColor }
        if option == hovered { return theme.buttonHoverColor }
        return .clear
    }
}
