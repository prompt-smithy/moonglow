import CoreFoundation
import CoreGraphics
import Foundation

/// Native macOS backends for display enumeration, colour temperature, and brightness.
///
/// The pure halves of this work live in ``KelvinConversion`` and ``DDCPacket`` and are unit tested.
/// Everything in this file touches hardware — CoreGraphics gamma tables, and DDC/CI over I2C — so it
/// can only be validated against a real display.
///
/// ## Public CoreGraphics (headers exist, called directly)
///
/// `CGGetActiveDisplayList`, `CGMainDisplayID`, `CGDisplayIsBuiltin`, `CGDisplayGammaTableCapacity`,
/// `CGSetDisplayTransferByTable`, `CGDisplayRestoreColorSyncSettings`.
///
/// ## Private CoreDisplay (no headers — resolved at runtime)
///
/// `IOAVServiceCreate`, `IOAVServiceReadI2C`, and `IOAVServiceWriteI2C` are the only way to drive
/// DDC/CI on Apple Silicon. They are bound in ``CoreDisplayBridge`` with `dlopen`/`dlsym`, which is
/// why Night Mode Controller is distributed directly rather than through the Mac App Store.
///
/// The service object they return is a Core Foundation value: it is held as an `Unmanaged`
/// reference and released explicitly, or it leaks.

/// Runtime binding for the private CoreDisplay DDC/CI entry points.
private enum CoreDisplayBridge {

    typealias CreateFunction = @convention(c) (CFAllocator?) -> UnsafeMutableRawPointer?

    typealias ReadI2CFunction = @convention(c) (
        UnsafeMutableRawPointer?,
        UInt32,
        UInt32,
        UnsafeMutablePointer<UInt8>?,
        UInt32
    ) -> Int32

    typealias WriteI2CFunction = @convention(c) (
        UnsafeMutableRawPointer?,
        UInt32,
        UInt32,
        UnsafeMutablePointer<UInt8>?,
        UInt32
    ) -> Int32

    private static let handle = dlopen(
        "/System/Library/Frameworks/CoreDisplay.framework/CoreDisplay",
        RTLD_NOW
    )

    static let create: CreateFunction? = resolve("IOAVServiceCreate")
    static let readI2C: ReadI2CFunction? = resolve("IOAVServiceReadI2C")
    static let writeI2C: WriteI2CFunction? = resolve("IOAVServiceWriteI2C")

    /// True only when every symbol the DDC backend needs is present.
    static var isAvailable: Bool { create != nil && readI2C != nil && writeI2C != nil }

    private static func resolve<T>(_ name: String) -> T? {
        guard let handle, let symbol = dlsym(handle, name) else { return nil }
        return unsafeBitCast(symbol, to: T.self)
    }

    /// Releases a service returned by `IOAVServiceCreate`, which hands back a +1 reference.
    static func release(_ service: UnsafeMutableRawPointer) {
        Unmanaged<AnyObject>.fromOpaque(service).release()
    }
}

// MARK: - Display enumeration

/// Enumerates displays through CoreGraphics.
public final class CoreGraphicsDisplayService: DisplayService {

    /// Whether the DDC/CI probe succeeded.
    ///
    /// Threaded in rather than probed here, because a capability must never be reported unless the
    /// backend that acts on it can actually apply it — the brightness backend owns that answer.
    private let ddcBrightnessSupported: Bool

    public private(set) var displays: [DisplayInfo] = []

    public init(ddcBrightnessSupported: Bool = false) {
        self.ddcBrightnessSupported = ddcBrightnessSupported
    }

    /// Re-enumerates the active displays.
    @discardableResult
    public func refresh() -> [DisplayInfo] {
        var count: UInt32 = 0
        guard CGGetActiveDisplayList(0, nil, &count) == .success, count > 0 else {
            displays = []
            return displays
        }

        var ids = [CGDirectDisplayID](repeating: 0, count: Int(count))
        var actualCount: UInt32 = 0
        guard CGGetActiveDisplayList(count, &ids, &actualCount) == .success else {
            displays = []
            return displays
        }

        let mainDisplay = CGMainDisplayID()

        displays = ids.prefix(Int(actualCount)).enumerated().map { index, id in
            let isPrimary = id == mainDisplay

            // Gamma support is what colour temperature needs: a capacity of 0 means the display
            // cannot take transfer tables at all.
            let supportsGamma = CGDisplayGammaTableCapacity(id) > 0

            // DDC/CI only reaches an external panel, and only the primary display is addressed.
            let isBuiltin = CGDisplayIsBuiltin(id) != 0

            return DisplayInfo(
                nativeId: id,
                name: isPrimary ? "Built-in or primary display" : "External display \(index + 1)",
                identifier: String(id),
                isPrimary: isPrimary,
                supportsBrightness: ddcBrightnessSupported && isPrimary && !isBuiltin,
                supportsColorTemperature: supportsGamma
            )
        }

        return displays
    }
}

// MARK: - Colour temperature

/// Applies colour temperature by writing per-channel gamma transfer tables.
public final class CoreGraphicsColorTemperatureService: ColorTemperatureService {

    private let displayService: DisplayService

    /// CoreGraphics is serialized behind a lock as the .NET original was: two overlapping writes can
    /// otherwise interleave the ramps of two different temperatures.
    private let lock = NSLock()

    public init(displayService: DisplayService) {
        self.displayService = displayService
    }

    /// The mechanism always exists on macOS. Whether a *display* accepts it is reported per display
    /// through `DisplayInfo.supportsColorTemperature`.
    public var isSupported: Bool { true }

    /// Applies `kelvin` to every display that accepts gamma tables.
    ///
    /// Returns whether at least one display took the ramp, so a caller can tell "applied" from "no
    /// capable display attached".
    @discardableResult
    public func apply(kelvin: Int) -> Bool {
        lock.lock()
        defer { lock.unlock() }

        let scale = KelvinConversion.rgbScale(kelvin: kelvin)
        var applied = false

        for display in displayService.displays where display.supportsColorTemperature {
            let capacity = Int(CGDisplayGammaTableCapacity(display.nativeId))
            guard capacity > 0 else { continue }

            let ramp = KelvinConversion.buildRamp(
                capacity: capacity,
                redScale: scale.red,
                greenScale: scale.green,
                blueScale: scale.blue
            )

            if setTransferTable(for: display.nativeId, ramp: ramp, capacity: capacity) {
                applied = true
            }
        }

        return applied
    }

    /// Restores the ColorSync state, undoing every ramp this service applied.
    ///
    /// Called on the way out, so quitting Night Mode Controller never leaves a display tinted.
    public func reset() {
        CGDisplayRestoreColorSyncSettings()
    }

    private func setTransferTable(
        for display: CGDirectDisplayID,
        ramp: (red: [Float], green: [Float], blue: [Float]),
        capacity: Int
    ) -> Bool {
        let error = ramp.red.withUnsafeBufferPointer { red in
            ramp.green.withUnsafeBufferPointer { green in
                ramp.blue.withUnsafeBufferPointer { blue in
                    CGSetDisplayTransferByTable(
                        display,
                        UInt32(capacity),
                        red.baseAddress,
                        green.baseAddress,
                        blue.baseAddress
                    )
                }
            }
        }

        return error == .success
    }
}

// MARK: - Hardware brightness

/// DDC/CI brightness for Apple Silicon, over the private CoreDisplay `IOAVService*` API.
public final class AppleSiliconBrightnessService: BrightnessService {

    /// Serializes I2C traffic. The MCCS protocol needs settling time between transactions, so writes
    /// are never issued concurrently.
    private let queue = DispatchQueue(label: "com.amber.ddc")

    /// Guards `service`, `pendingBrightness`, and `isDisposed`.
    private let lock = NSLock()

    private var service: UnsafeMutableRawPointer?
    private var pendingBrightness = AppSettings.defaultManualBrightness
    private var pendingWrite: DispatchWorkItem?
    private var isDisposed = false

    public init() {
        #if arch(arm64)
        guard CoreDisplayBridge.isAvailable, let created = CoreDisplayBridge.create?(nil) else {
            return
        }

        service = created

        // An open service is not proof that the panel speaks DDC/CI: the luminance probe decides,
        // and a panel that does not answer must not be reported as brightness-capable.
        if !probeBrightness() {
            CoreDisplayBridge.release(created)
            service = nil
        }
        #endif
    }

    public var isSupported: Bool {
        lock.lock()
        defer { lock.unlock() }
        return service != nil
    }

    /// Queues a brightness write, coalescing rapid slider changes into one I2C transaction.
    public func setBrightness(percent: Int) {
        lock.lock()

        guard service != nil, !isDisposed else {
            lock.unlock()
            return
        }

        pendingBrightness = AppSettings.clampBrightness(percent)
        pendingWrite?.cancel()

        let work = DispatchWorkItem { [weak self] in
            self?.applyPendingBrightness()
        }
        pendingWrite = work
        lock.unlock()

        queue.asyncAfter(deadline: .now() + DDCPacket.debounceDelay, execute: work)
    }

    /// Releases the CoreDisplay service. Called on the way out.
    public func dispose() {
        lock.lock()
        defer { lock.unlock() }

        guard !isDisposed else { return }
        isDisposed = true

        pendingWrite?.cancel()
        pendingWrite = nil

        if let service {
            CoreDisplayBridge.release(service)
            self.service = nil
        }
    }

    // MARK: - DDC traffic

    /// Sends the MCCS luminance query and waits for a reply. A panel that answers can take the
    /// luminance writes that follow.
    private func probeBrightness() -> Bool {
        for _ in 0..<DDCPacket.maxAttempts {
            Thread.sleep(forTimeInterval: DDCPacket.writeDelay)
            if !write(DDCPacket.probeRequest) {
                return false
            }
        }

        Thread.sleep(forTimeInterval: DDCPacket.writeDelay)
        return read(byteCount: 12)
    }

    private func applyPendingBrightness() {
        lock.lock()
        let target = service
        let isDisposed = isDisposed
        let percent = pendingBrightness
        lock.unlock()

        guard let target, !isDisposed else { return }

        // Two attempts, as the original did: DDC/CI is lossy and a single dropped transaction
        // should not leave the panel at the old brightness.
        let packet = DDCPacket.buildWritePacket(percent: percent)
        for _ in 0..<DDCPacket.maxAttempts {
            Thread.sleep(forTimeInterval: DDCPacket.writeDelay)
            if !write(packet, to: target) {
                return
            }
        }
    }

    private func write(_ bytes: [UInt8], to target: UnsafeMutableRawPointer? = nil) -> Bool {
        guard let writeI2C = CoreDisplayBridge.writeI2C else { return false }

        lock.lock()
        let service = target ?? self.service
        lock.unlock()

        guard let service else { return false }

        var payload = bytes
        let result = payload.withUnsafeMutableBufferPointer { buffer in
            writeI2C(
                service,
                DDCPacket.chipAddress,
                DDCPacket.dataAddress,
                buffer.baseAddress,
                UInt32(buffer.count)
            )
        }

        return result == 0
    }

    private func read(byteCount: Int) -> Bool {
        guard let readI2C = CoreDisplayBridge.readI2C else { return false }

        lock.lock()
        let service = self.service
        lock.unlock()

        guard let service else { return false }

        var reply = [UInt8](repeating: 0, count: byteCount)
        let result = reply.withUnsafeMutableBufferPointer { buffer in
            readI2C(
                service,
                DDCPacket.chipAddress,
                DDCPacket.dataAddress,
                buffer.baseAddress,
                UInt32(buffer.count)
            )
        }

        return result == 0
    }
}
