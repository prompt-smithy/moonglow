import SwiftUI

/// The warm-to-cool ramp that every colour-temperature control is drawn on.
///
/// These are fixed colours rather than palette entries: the ramp has to read as "warm at the
/// night end, cool at the day end" whichever theme is active, and the palette accents do not
/// carry that meaning. Shared between the popup's single-handle slider and the settings
/// window's two-handle range slider so the two cannot drift apart.
enum TemperatureGradient {

    /// Night end first, day end last.
    static let colors: [Color] = [Color(hex: "#FFA94D"), Color(hex: "#8FB8FF")]

    static var linear: LinearGradient {
        LinearGradient(colors: colors, startPoint: .leading, endPoint: .trailing)
    }
}
