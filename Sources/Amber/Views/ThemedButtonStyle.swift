import SwiftUI
import AmberKit

/// A push button drawn from the active palette.
///
/// SwiftUI's own button styles take their surface from the system, so inside an Amber palette the
/// popup's actions came out system grey. Drawing them here keeps them on-palette, the same way
/// ``ThemedDropDown`` draws its own field rather than accepting the system bezel.
struct ThemedButtonStyle: ButtonStyle {

    /// Which surface the button sits on.
    enum Emphasis {
        /// Filled with the palette accent — for the primary action of a group.
        case prominent
        /// Palette control surface with a hairline border — for everything else.
        case standard
    }

    let emphasis: Emphasis
    let theme: AppTheme

    func makeBody(configuration: ButtonStyleConfiguration) -> some View {
        ThemedButtonBody(emphasis: emphasis, theme: theme, configuration: configuration)
    }

    /// Split out so hover state has somewhere to live: `makeBody` is not itself a `View` and so
    /// cannot hold `@State`. Named `ThemedButtonBody` rather than `Body` because `Body` is already
    /// a requirement of `ButtonStyle`.
    private struct ThemedButtonBody: View {

        let emphasis: Emphasis
        let theme: AppTheme
        let configuration: ButtonStyleConfiguration

        @State private var isHovering = false

        private var shape: RoundedRectangle {
            RoundedRectangle(cornerRadius: 6, style: .continuous)
        }

        var body: some View {
            configuration.label
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(foreground)
                .padding(.horizontal, 12)
                .padding(.vertical, 5)
                .background(shape.fill(background))
                .overlay {
                    shape.strokeBorder(border, lineWidth: 1)
                }
                .contentShape(shape)
                .onHover { isHovering = $0 }
        }

        private var foreground: Color {
            switch emphasis {
            case .prominent: theme.onAccentTextColor
            case .standard: theme.primaryTextColor
            }
        }

        private var background: Color {
            switch emphasis {
            case .prominent:
                // Dimming the fill is the only press feedback available on an accent surface:
                // nudging the text colour would cost contrast.
                configuration.isPressed ? theme.accentColor.opacity(0.75) : theme.accentColor
            case .standard:
                isHovering || configuration.isPressed
                    ? theme.buttonHoverColor
                    : theme.controlBackgroundColor
            }
        }

        private var border: Color {
            switch emphasis {
            case .prominent: theme.accentColor
            case .standard: theme.borderColor
            }
        }
    }
}
