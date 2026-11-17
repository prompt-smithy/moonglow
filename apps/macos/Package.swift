// swift-tools-version: 6.0
import PackageDescription
import Foundation

let sandboxPrototype = ProcessInfo.processInfo.environment["MOONGLOW_APP_SANDBOXED"] == "1"
let appSwiftSettings: [SwiftSetting] = [.swiftLanguageMode(.v5)]
    + (sandboxPrototype ? [.define("MOONGLOW_APP_SANDBOXED")] : [])
let appLinkerSettings: [LinkerSetting] = sandboxPrototype ? [] : [.linkedFramework("IOKit")]

// Moonglow — macOS menu-bar app for external-display brightness and colour temperature.
//
// Layout:
//   MoonglowKit  — platform logic: models, schedule math, theme catalog, settings, service protocols.
//   Moonglow     — macOS app shell: status item, windows, SwiftUI views, native bridges.
//   MoonglowKitTests — unit tests for everything in MoonglowKit.
//
// Language mode is pinned to v5 while the app shell and native bridge are still being ported.
// Tighten to Swift 6 mode once the CoreGraphics/CoreDisplay bridge is in place.
let package = Package(
    name: "Moonglow",
    platforms: [
        .macOS(.v12)
    ],
    products: [
        .executable(name: "Moonglow", targets: ["Moonglow"]),
        .library(name: "MoonglowKit", targets: ["MoonglowKit"]),
    ],
    targets: [
        .target(
            name: "MoonglowKit",
            swiftSettings: appSwiftSettings,
            linkerSettings: appLinkerSettings
        ),
        .executableTarget(
            name: "Moonglow",
            dependencies: ["MoonglowKit"],
            resources: [.process("Resources")],
            swiftSettings: appSwiftSettings
        ),
        .testTarget(
            name: "MoonglowKitTests",
            dependencies: ["MoonglowKit"],
            swiftSettings: appSwiftSettings
        ),
    ]
)
