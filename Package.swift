// swift-tools-version: 6.0
import PackageDescription

// Amber — macOS menu-bar app for external-display brightness and colour temperature.
//
// Layout:
//   AmberKit  — platform logic: models, schedule math, theme catalog, settings, service protocols.
//   Amber     — macOS app shell: status item, windows, SwiftUI views, native bridges.
//   AmberKitTests — unit tests for everything in AmberKit.
//
// Language mode is pinned to v5 while the app shell and native bridge are still being ported.
// Tighten to Swift 6 mode once the CoreGraphics/CoreDisplay bridge is in place.
let package = Package(
    name: "Amber",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .executable(name: "Amber", targets: ["Amber"]),
        .library(name: "AmberKit", targets: ["AmberKit"]),
    ],
    targets: [
        .target(
            name: "AmberKit",
            swiftSettings: [.swiftLanguageMode(.v5)]
        ),
        .executableTarget(
            name: "Amber",
            dependencies: ["AmberKit"],
            swiftSettings: [.swiftLanguageMode(.v5)]
        ),
        .testTarget(
            name: "AmberKitTests",
            dependencies: ["AmberKit"],
            swiftSettings: [.swiftLanguageMode(.v5)]
        ),
    ]
)
