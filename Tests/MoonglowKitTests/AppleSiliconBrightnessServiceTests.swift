import Foundation
import Testing
@testable import MoonglowKit

#if arch(arm64)
// The recovery logic only runs on Apple Silicon: elsewhere the probe is compiled out
// and hardware is always down.

/// Covers the DDC recovery path: a handle gone stale after a display reconfiguration
/// must heal with a fresh handle, and a silent panel must report hardware down so the
/// gamma fallback takes over — never write to a dead handle for the rest of the session.
@Suite("Apple Silicon DDC recovery")
struct AppleSiliconBrightnessServiceTests {

    /// Well-formed GetVCP reply: current = 50, max = 100.
    fileprivate static let probeReply: [UInt8] =
        [0x6E, 0x88, 0x02, 0x00, 0x10, 0x00, 0x00, 0x64, 0x00, 0x32, 0xF2]

    @Test("A stale handle recovers with a fresh handle and keeps hardware up")
    func staleHandleRecovers() async {
        let transport = FakeDDCTransport()
        let service = AppleSiliconBrightnessService(transport: transport)
        service.refreshSupport()
        #expect(service.isSupported)
        #expect(transport.created.count == 1)
        let stale = transport.created

        // The reconfiguration happens: the old handle stops answering brightness writes.
        transport.staleHandles = Set(stale)
        service.setBrightness(percent: 70)

        #expect(await waitUntil { transport.brightnessWrites.count == 3 })
        #expect(service.isSupported)
        #expect(transport.created.count == 2)
        #expect(transport.released == stale)
        let fresh = transport.created[1]
        #expect(transport.brightnessWrites.allSatisfy { $0.packet == DDCPacket.buildWritePacket(percent: 70) })
        #expect(transport.brightnessWrites.last?.handle == fresh)
    }

    @Test("A silent panel reports hardware down, and returns on the next refresh")
    func silentPanelReportsHardwareDown() async {
        let transport = FakeDDCTransport()
        let service = AppleSiliconBrightnessService(transport: transport)
        service.refreshSupport()
        #expect(service.isSupported)

        // The panel goes away mid-session: writes fail and the re-probe hears nothing.
        transport.staleHandles = Set(transport.created)
        transport.panelAnswers = false
        service.setBrightness(percent: 70)

        #expect(await waitUntil(timeout: 10) { !service.isSupported })
        #expect(transport.created.count == 2)
        #expect(transport.released.sorted() == transport.created.sorted())

        // The panel answers again: the next refresh brings hardware back, and the
        // completion tells the view model it newly did.
        transport.panelAnswers = true
        transport.staleHandles = []
        let newlyAvailable = LockedBox<Bool>()
        service.refreshSupportAsync { value in newlyAvailable.value = value }

        #expect(await waitUntil { newlyAvailable.value == true })
        #expect(service.isSupported)
        #expect(transport.created.count == 3)
    }

    @Test("Dispose releases the service and ignores later writes")
    func disposeReleasesService() {
        let transport = FakeDDCTransport()
        let service = AppleSiliconBrightnessService(transport: transport)
        service.refreshSupport()
        #expect(service.isSupported)

        service.dispose()

        #expect(!service.isSupported)
        #expect(transport.released == transport.created)
        let writeCount = transport.writes.count
        service.setBrightness(percent: 70)
        #expect(transport.writes.count == writeCount)
    }
}

/// Fake I2C transport with distinct fake handles. Probe writes always address (the
/// addressing phase is not what goes stale); brightness writes fail on stale handles;
/// reads fail when the panel is silent.
private final class FakeDDCTransport: DDCTransport {
    private let lock = NSLock()
    private var _isAvailable = true
    private var _nextHandle = 1
    private var _created: [Int] = []
    private var _released: [Int] = []
    private var _writes: [(handle: Int, packet: [UInt8])] = []
    private var _staleHandles: Set<Int> = []
    private var _panelAnswers = true

    var isAvailable: Bool {
        get { locked { _isAvailable } }
        set { locked { _isAvailable = newValue } }
    }

    var staleHandles: Set<Int> {
        get { locked { _staleHandles } }
        set { locked { _staleHandles = newValue } }
    }

    var panelAnswers: Bool {
        get { locked { _panelAnswers } }
        set { locked { _panelAnswers = newValue } }
    }

    var created: [Int] { locked { _created } }
    var released: [Int] { locked { _released } }
    var writes: [(handle: Int, packet: [UInt8])] { locked { _writes } }

    /// Recorded writes excluding the luminance probe addressing.
    var brightnessWrites: [(handle: Int, packet: [UInt8])] {
        locked { _writes.filter { $0.packet != DDCPacket.probeRequest } }
    }

    func createService() -> UnsafeMutableRawPointer? {
        locked {
            guard _isAvailable else { return nil }
            let id = _nextHandle
            _nextHandle += 1
            _created.append(id)
            return UnsafeMutableRawPointer(bitPattern: id)
        }
    }

    func releaseService(_ service: UnsafeMutableRawPointer) {
        locked { _released.append(Int(bitPattern: service)) }
    }

    func write(_ bytes: [UInt8], to service: UnsafeMutableRawPointer) -> Bool {
        locked {
            let id = Int(bitPattern: service)
            _writes.append((id, bytes))
            if bytes == DDCPacket.probeRequest { return true }
            return !_staleHandles.contains(id)
        }
    }

    func read(byteCount: Int, from service: UnsafeMutableRawPointer) -> [UInt8]? {
        locked {
            guard _panelAnswers else { return nil }
            return AppleSiliconBrightnessServiceTests.probeReply
        }
    }

    private func locked<T>(_ body: () -> T) -> T {
        lock.lock()
        defer { lock.unlock() }
        return body()
    }
}

/// Carries one value across the queue hop to the async test.
private final class LockedBox<T: Sendable>: @unchecked Sendable {
    private let lock = NSLock()
    private var _value: T?

    var value: T? {
        get { lock.lock(); defer { lock.unlock() }; return _value }
        set { lock.lock(); defer { lock.unlock() }; _value = newValue }
    }
}

/// Polls until `condition` holds or the timeout expires. The service debounces,
/// delays, and retries with real sleeps, so the recovery path settles in ~0.5 s.
private func waitUntil(
    timeout: TimeInterval = 5,
    _ condition: () -> Bool
) async -> Bool {
    let deadline = Date().addingTimeInterval(timeout)
    while !condition() {
        if Date() > deadline { return false }
        try? await Task.sleep(nanoseconds: 10_000_000)
    }
    return true
}
#endif
