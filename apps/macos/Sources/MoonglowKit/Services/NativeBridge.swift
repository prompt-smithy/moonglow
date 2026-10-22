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
/// why Moonglow is distributed directly rather than through the Mac App Store.
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
    /// Read live on every refresh rather than snapshotted, because a panel may answer late and a
    /// capability must never be reported unless the backend that acts on it can actually apply
    /// it — the brightness backend owns that answer.
    private let ddcBrightnessSupported: () -> Bool

    public private(set) var displays: [DisplayInfo] = []

    public init(ddcBrightnessSupported: @escaping () -> Bool = { false }) {
        self.ddcBrightnessSupported = ddcBrightnessSupported
    }

    /// Re-enumerates the active displays.
    @discardableResult
    public func refresh() -> [DisplayInfo] {
        var count: UInt32 = 0
        guard CGGetActiveDisplayList(0, nil, &count) == .success, count > 0 else {
            displays = []
            AppLog.shared.log(.display, "display refresh: no active displays")
            return displays
        }

        var ids = [CGDirectDisplayID](repeating: 0, count: Int(count))
        var actualCount: UInt32 = 0
        guard CGGetActiveDisplayList(count, &ids, &actualCount) == .success else {
            displays = []
            AppLog.shared.log(.display, "display refresh: active display list unreadable")
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
                isPrimary: isPrimary,
                supportsBrightness: ddcBrightnessSupported()
                    ? isPrimary && !isBuiltin
                    : supportsGamma,
                supportsColorTemperature: supportsGamma
            )
        }

        let summary = displays.map {
            "id=\($0.nativeId)\($0.isPrimary ? " primary" : "")"
                + " brightness=\($0.supportsBrightness) color=\($0.supportsColorTemperature)"
        }.joined(separator: "; ")
        AppLog.shared.log(.display, "displays: \(summary)")

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
    private var currentKelvin = 6500
    private var softwareBrightness = 100

    /// The ramp currently on screen, if every target display took it. The schedule
    /// re-evaluates every 10 s but the temperature only moves during the two transitions,
    /// so identical writes are skipped instead of re-uploading a ramp every tick.
    /// ``cacheLifetime`` bounds how long the skip lasts, and ``invalidate()`` ends it early.
    private var lastApplied: (kelvin: Int, brightness: Int, displays: Set<CGDirectDisplayID>)?

    /// When ``lastApplied`` was written, so the cache can expire. See ``cacheLifetime``.
    private var lastAppliedAt: Date?

    /// How long a cached ramp is trusted before it is written again, unchanged.
    ///
    /// The gamma tables are global: another process can replace or clear them at any
    /// time and nothing tells us when it does. Expiring the cache means the schedule's
    /// 10 s tick re-asserts the ramp within two minutes instead of trusting it forever —
    /// a small fraction of the pre-cache write rate, and self-healing.
    private static let cacheLifetime: TimeInterval = 120

    public init(displayService: DisplayService) {
        self.displayService = displayService
    }

    /// The mechanism always exists on macOS. Whether a *display* accepts it is reported per display
    /// through `DisplayInfo.supportsColorTemperature`.
    public var isSupported: Bool { true }

    /// Applies `kelvin` and the current software brightness to every display that accepts gamma tables.
    ///
    /// Returns whether at least one display took the ramp, so a caller can tell "applied" from "no
    /// capable display attached".
    @discardableResult
    public func apply(kelvin: Int) -> Bool {
        lock.lock()
        defer { lock.unlock() }

        currentKelvin = kelvin
        let applied = applyCurrentRamp()
        if !applied {
            // Failures only: the schedule applies every 10 seconds, so successes would flood.
            AppLog.shared.log(.display, "gamma apply failed for \(kelvin)K: no display took the ramp")
        }
        return applied
    }

    /// Whether at least one active display can use a gamma ramp for software brightness.
    public var supportsSoftwareBrightness: Bool {
        displayService.displays.contains { $0.supportsColorTemperature }
    }

    /// Applies software dimming through the same gamma ramp as colour temperature.
    public func setSoftwareBrightness(percent: Int) {
        lock.lock()
        defer { lock.unlock() }

        softwareBrightness = AppSettings.clampBrightness(percent)
        _ = applyCurrentRamp()
    }

    private func applyCurrentRamp() -> Bool {
        let targets = displayService.displays.filter { $0.supportsColorTemperature }
        guard !targets.isEmpty else {
            clearCache()
            return false
        }

        let ids = Set(targets.map(\.nativeId))
        if let lastApplied,
           let lastAppliedAt,
           Date().timeIntervalSince(lastAppliedAt) < Self.cacheLifetime,
           lastApplied.kelvin == currentKelvin,
           lastApplied.brightness == softwareBrightness,
           lastApplied.displays == ids
        {
            return true
        }

        let scale = KelvinConversion.rgbScale(kelvin: currentKelvin)
        let brightnessScale = Double(softwareBrightness) / 100.0
        var applied = false
        var allApplied = true

        for display in targets {
            let capacity = Int(CGDisplayGammaTableCapacity(display.nativeId))
            guard capacity > 0 else {
                // Capacity can return after sleep/wake; never cache a partial ramp or
                // the recovered display would stay untinted.
                allApplied = false
                continue
            }

            let ramp = KelvinConversion.buildRamp(
                capacity: capacity,
                redScale: scale.red,
                greenScale: scale.green,
                blueScale: scale.blue,
                brightnessScale: brightnessScale
            )

            if setTransferTable(for: display.nativeId, ramp: ramp, capacity: capacity) {
                applied = true
            } else {
                allApplied = false
            }
        }

        if allApplied {
            lastApplied = (currentKelvin, softwareBrightness, ids)
            lastAppliedAt = Date()
        }
        return applied
    }

    /// Forgets the cached ramp. Called with ``lock`` held.
    private func clearCache() {
        lastApplied = nil
        lastAppliedAt = nil
    }

    /// Drops the cached ramp so the next apply writes it again. See the protocol.
    public func invalidate() {
        lock.lock()
        defer { lock.unlock() }
        clearCache()
    }

    /// Restores the ColorSync state, undoing every ramp this service applied.
    ///
    /// Called on the way out, so quitting Moonglow never leaves a display tinted.
    public func reset() {
        lock.lock()
        defer { lock.unlock() }
        clearCache()
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

/// Software brightness fallback backed by the shared temperature gamma ramps.
public final class CoreGraphicsBrightnessService: BrightnessService {

    private let colorService: CoreGraphicsColorTemperatureService

    public init(colorService: CoreGraphicsColorTemperatureService) {
        self.colorService = colorService
    }

    public var isSupported: Bool { colorService.supportsSoftwareBrightness }

    public func setBrightness(percent: Int) {
        colorService.setSoftwareBrightness(percent: percent)
    }
}

// MARK: - Hardware brightness

/// DDC/CI transaction transport behind ``AppleSiliconBrightnessService``.
///
/// Production talks to the panel through the private CoreDisplay entry points; tests
/// inject a fake, which is the only way to exercise the recovery path — a handle gone
/// stale after a display reconfiguration, or a panel that stops answering — without
/// hardware.
protocol DDCTransport: AnyObject {
    /// False when the backend itself is missing (symbols gone), as opposed to silent hardware.
    var isAvailable: Bool { get }
    func createService() -> UnsafeMutableRawPointer?
    func releaseService(_ service: UnsafeMutableRawPointer)
    func write(_ bytes: [UInt8], to service: UnsafeMutableRawPointer) -> Bool
    func read(byteCount: Int, from service: UnsafeMutableRawPointer) -> [UInt8]?
}

/// Production ``DDCTransport`` over the private CoreDisplay `IOAVService*` API.
final class CoreDisplayDDCTransport: DDCTransport {

    var isAvailable: Bool { CoreDisplayBridge.isAvailable }

    func createService() -> UnsafeMutableRawPointer? {
        CoreDisplayBridge.create?(nil)
    }

    func releaseService(_ service: UnsafeMutableRawPointer) {
        CoreDisplayBridge.release(service)
    }

    func write(_ bytes: [UInt8], to service: UnsafeMutableRawPointer) -> Bool {
        guard let writeI2C = CoreDisplayBridge.writeI2C else { return false }

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

    func read(byteCount: Int, from service: UnsafeMutableRawPointer) -> [UInt8]? {
        guard let readI2C = CoreDisplayBridge.readI2C else { return nil }

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

        return result == 0 ? reply : nil
    }
}

/// DDC/CI brightness for Apple Silicon, over the private CoreDisplay `IOAVService*` API.
public final class AppleSiliconBrightnessService: BrightnessService {

    /// Serializes I2C traffic. The MCCS protocol needs settling time between transactions, so writes
    /// are never issued concurrently.
    private let queue = DispatchQueue(label: "com.prompt-smithy.Moonglow.ddc")

    /// Guards `service`, `pendingBrightness`, and `isDisposed`.
    private let lock = NSLock()

    private let transport: DDCTransport

    private var service: UnsafeMutableRawPointer?
    private var pendingBrightness = AppSettings.defaultManualBrightness
    private var pendingWrite: DispatchWorkItem?
    private var isDisposed = false

    public init() {
        self.transport = CoreDisplayDDCTransport()
        // The first probe runs through refreshSupportAsync from the view model, off the
        // main thread: probing here would freeze launch for up to ~0.45 s on a silent
        // panel, and a second probe from the first display refresh would double it.
    }

    /// Test seam: drives the probe/write/recovery logic against a fake transport.
    init(transport: DDCTransport) {
        self.transport = transport
    }

    public var isSupported: Bool {
        lock.lock()
        defer { lock.unlock() }
        return service != nil
    }

    /// Opens the CoreDisplay service and probes it, unless hardware is already up.
    public func refreshSupport() {
        tryOpenAndProbe()
    }

    /// Probes on the serial queue so I2C traffic — probe and brightness writes alike —
    /// is never issued concurrently, and the calling thread never sleeps.
    public func refreshSupportAsync(completion: (@Sendable (Bool) -> Void)?) {
        queue.async { [weak self] in
            completion?(self?.tryOpenAndProbe() ?? false)
        }
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
        guard !isDisposed else {
            lock.unlock()
            return
        }
        isDisposed = true
        pendingWrite?.cancel()
        pendingWrite = nil
        lock.unlock()

        // A cancelled write that is already executing keeps running, and a probe may be
        // mid-round on this same queue. Drain it before releasing the service, or quit
        // within ~90 ms of a brightness change reaches I2C with a freed pointer.
        queue.sync {}

        lock.lock()
        defer { lock.unlock() }
        if let service {
            transport.releaseService(service)
            self.service = nil
        }
    }

    // MARK: - DDC traffic

    /// Opens the CoreDisplay service and probes it, unless hardware is already up.
    ///
    /// Returns true when this call newly established support. Safe from any thread; the
    /// async entry point runs it on the serial queue so probe traffic never overlaps a
    /// brightness write.
    @discardableResult
    private func tryOpenAndProbe() -> Bool {
        #if arch(arm64)
        lock.lock()
        guard service == nil, !isDisposed else {
            lock.unlock()
            return false
        }
        lock.unlock()

        guard transport.isAvailable else {
            AppLog.shared.log(.brightness, "DDC probe skipped: CoreDisplay symbols unavailable")
            return false
        }
        guard let created = transport.createService() else {
            AppLog.shared.log(.brightness, "DDC probe failed: could not create IOAVService")
            return false
        }

        lock.lock()
        guard service == nil, !isDisposed else {
            lock.unlock()
            transport.releaseService(created)
            return false
        }
        service = created
        lock.unlock()

        // An open service is not proof that the panel speaks DDC/CI: the luminance probe decides,
        // and a panel that does not answer must not be reported as brightness-capable.
        if !probeBrightness() {
            AppLog.shared.log(.brightness, "DDC hardware brightness unavailable, using software fallback")
            lock.lock()
            let stillOurs = service.map { $0 == created } ?? false
            if stillOurs {
                service = nil
            }
            lock.unlock()
            // A dispose mid-probe already released this service; only release what is still ours.
            if stillOurs {
                transport.releaseService(created)
            }
            return false
        }
        return true
        #else
        return false
        #endif
    }

    /// Sends the MCCS luminance query and validates the reply. Only a panel that answers with
    /// a well-formed GetVCP response can take the luminance writes that follow — a completed
    /// read alone is not proof, and claiming support without it leaves the slider writing to a
    /// panel that never listens while the working software fallback sits unused.
    ///
    /// Retried over several rounds: a capable panel may drop the first transactions while its
    /// DDC bridge wakes up, and one lost round must not cost hardware control for the session.
    private func probeBrightness() -> Bool {
        for round in 1...DDCPacket.probeRounds {
            let tag = "round \(round)/\(DDCPacket.probeRounds)"
            var addressed = false
            for _ in 0..<DDCPacket.maxAttempts {
                Thread.sleep(forTimeInterval: DDCPacket.writeDelay)
                if write(DDCPacket.probeRequest) {
                    addressed = true
                    break
                }
            }
            guard addressed else {
                AppLog.shared.log(.brightness, "DDC probe \(tag): luminance query not accepted")
                continue
            }

            Thread.sleep(forTimeInterval: DDCPacket.writeDelay)
            guard let reply = read(byteCount: DDCPacket.probeReplyLength) else {
                AppLog.shared.log(.brightness, "DDC probe \(tag): no reply")
                continue
            }
            guard DDCPacket.isValidProbeReply(reply) else {
                AppLog.shared.log(.brightness, "DDC probe \(tag): invalid reply [\(hex(reply))]")
                continue
            }
            AppLog.shared.log(.brightness, "DDC hardware brightness available (probe \(tag))")
            return true
        }
        return false
    }

    private func applyPendingBrightness() {
        lock.lock()
        let target = service
        let isDisposed = isDisposed
        let percent = pendingBrightness
        lock.unlock()

        guard let target, !isDisposed else { return }

        if writeWithRetries(DDCPacket.buildWritePacket(percent: percent), to: target) {
            return
        }
        AppLog.shared.log(
            .brightness,
            "DDC write failed for \(percent)% after \(DDCPacket.maxAttempts) attempts"
        )
        recoverAndRetry(percent: percent)
    }

    /// Recovers from a handle that likely went stale: a display reconfiguration
    /// invalidates the service while the panel itself keeps answering, which only a
    /// fresh handle proves. The stale handle is dropped, the panel is probed again from
    /// scratch, and the write is retried once. When the panel does not answer either,
    /// hardware stays down (`service == nil`) so the fallback routes to gamma instead
    /// of writing to a dead handle; the next display refresh re-probes and can bring
    /// hardware back.
    private func recoverAndRetry(percent: Int) {
        releaseService()
        guard tryOpenAndProbe() else {
            AppLog.shared.log(
                .brightness,
                "DDC recovery failed; hardware brightness down until next refresh"
            )
            return
        }

        lock.lock()
        let target = service
        let disposed = isDisposed
        lock.unlock()

        guard let target, !disposed else { return }
        if writeWithRetries(DDCPacket.buildWritePacket(percent: percent), to: target) {
            AppLog.shared.log(.brightness, "DDC recovered with a fresh handle")
            return
        }
        AppLog.shared.log(
            .brightness,
            "DDC retry on fresh handle failed; hardware brightness down until next refresh"
        )
        releaseService()
    }

    /// Drops the service and reports hardware down, so the fallback routes to gamma.
    private func releaseService() {
        lock.lock()
        let old = service
        service = nil
        lock.unlock()
        if let old {
            transport.releaseService(old)
        }
    }

    /// One luminance write with the standard pre-transaction delay and retry budget:
    /// DDC/CI is lossy and a single dropped transaction should not leave the panel
    /// at the old brightness.
    private func writeWithRetries(_ packet: [UInt8], to target: UnsafeMutableRawPointer) -> Bool {
        for _ in 0..<DDCPacket.maxAttempts {
            Thread.sleep(forTimeInterval: DDCPacket.writeDelay)
            if write(packet, to: target) {
                return true
            }
        }
        return false
    }

    private func write(_ bytes: [UInt8], to target: UnsafeMutableRawPointer? = nil) -> Bool {
        lock.lock()
        let service = target ?? self.service
        lock.unlock()

        guard let service else { return false }
        return transport.write(bytes, to: service)
    }

    private func read(byteCount: Int) -> [UInt8]? {
        lock.lock()
        let service = self.service
        lock.unlock()

        guard let service else { return nil }
        return transport.read(byteCount: byteCount, from: service)
    }
}

private func hex(_ bytes: [UInt8]) -> String {
    bytes.map { String(format: "%02x", $0) }.joined(separator: " ")
}
