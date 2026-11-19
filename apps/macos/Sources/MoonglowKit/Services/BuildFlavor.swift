import Foundation

/// Which macOS build this binary is.
///
/// The Mac App Store build is compiled with `MOONGLOW_APP_SANDBOXED`: it is sandboxed, offers software
/// dimming only, and contains no private DDC/CI code. The direct and Homebrew build keeps hardware backlight control.
public enum BuildFlavor {
    /// True in the Mac App Store build.
    public static let isAppStore: Bool = {
        #if MOONGLOW_APP_SANDBOXED
        return true
        #else
        return false
        #endif
    }()

    /// Whether this build can drive hardware (DDC/CI) backlight control.
    public static var supportsHardwareBrightness: Bool { !isAppStore }
}
