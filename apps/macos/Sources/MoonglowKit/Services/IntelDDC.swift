#if arch(x86_64)
import CoreGraphics
import Foundation
import IOKit
import IOKit.graphics
import IOKit.i2c

private final class IntelI2CConnection {

    private let connection: IOI2CConnectRef
    private let replyTransactionType: IOOptionBits
    private var isClosed = false

    private init(connection: IOI2CConnectRef, replyTransactionType: IOOptionBits) {
        self.connection = connection
        self.replyTransactionType = replyTransactionType
    }

    deinit {
        close()
    }

    static func open(displayID: CGDirectDisplayID) -> IntelI2CConnection? {
        guard CGDisplayIsBuiltin(displayID) == 0,
              let framebuffer = matchingFramebuffer(for: displayID)
        else {
            return nil
        }
        defer { IOObjectRelease(framebuffer) }

        var busCount: IOItemCount = 0
        guard IOFBGetI2CInterfaceCount(framebuffer, &busCount) == KERN_SUCCESS else {
            return nil
        }

        let replyTypes = [
            IOOptionBits(kIOI2CDDCciReplyTransactionType),
            IOOptionBits(kIOI2CSimpleTransactionType),
        ]

        for bus in 0..<busCount {
            var interface: io_service_t = 0
            guard IOFBCopyI2CInterfaceForBus(framebuffer, bus, &interface) == KERN_SUCCESS else {
                continue
            }
            defer { IOObjectRelease(interface) }

            for replyType in replyTypes {
                var i2cConnection: IOI2CConnectRef?
                guard IOI2CInterfaceOpen(interface, 0, &i2cConnection) == KERN_SUCCESS,
                      let i2cConnection
                else {
                    continue
                }

                let candidate = IntelI2CConnection(
                    connection: i2cConnection,
                    replyTransactionType: replyType
                )
                if candidate.probeBrightness() {
                    return candidate
                }
                candidate.close()
            }
        }

        return nil
    }

    func writeBrightness(percent: Int) -> Bool {
        transact(send: DDCPacket.buildIntelWritePayload(percent: percent)) != nil
    }

    func close() {
        guard !isClosed else { return }
        isClosed = true
        _ = IOI2CInterfaceClose(connection, 0)
    }

    private func probeBrightness() -> Bool {
        guard let response = transact(
            send: DDCPacket.intelProbePayload,
            replyByteCount: DDCPacket.probeReplyLength
        ) else {
            return false
        }

        // The single tested validator — a local copy here already drifted once.
        return DDCPacket.isValidProbeReply(response)
    }

    private func transact(send: [UInt8], replyByteCount: Int = 0) -> [UInt8]? {
        var outgoing = send
        var incoming = [UInt8](repeating: 0, count: replyByteCount)
        var request = IOI2CRequest()
        request.sendTransactionType = IOOptionBits(kIOI2CSimpleTransactionType)
        request.replyTransactionType = replyByteCount > 0
            ? replyTransactionType
            : IOOptionBits(kIOI2CNoTransactionType)
        request.sendAddress = 0x6E
        request.replyAddress = 0x6F
        request.replySubAddress = UInt8(DDCPacket.dataAddress)
        request.minReplyDelay = 10
        request.sendBytes = UInt32(outgoing.count)
        request.replyBytes = UInt32(replyByteCount)

        let succeeded = outgoing.withUnsafeMutableBufferPointer { outgoingBuffer -> Bool in
            guard let outgoingBase = outgoingBuffer.baseAddress else { return false }
            request.sendBuffer = vm_address_t(bitPattern: outgoingBase)

            let sendRequest = {
                IOI2CSendRequest(self.connection, 0, &request) == KERN_SUCCESS
                    && request.result == KERN_SUCCESS
            }

            guard replyByteCount > 0 else { return sendRequest() }
            return incoming.withUnsafeMutableBufferPointer { incomingBuffer in
                guard let incomingBase = incomingBuffer.baseAddress else { return false }
                request.replyBuffer = vm_address_t(bitPattern: incomingBase)
                return sendRequest()
            }
        }

        return succeeded ? incoming : nil
    }

    private static func matchingFramebuffer(for displayID: CGDirectDisplayID) -> io_service_t? {
        let matching = IOServiceMatching("IOFramebuffer")
        var iterator: io_iterator_t = 0
        guard IOServiceGetMatchingServices(kIOMainPortDefault, matching, &iterator) == KERN_SUCCESS else {
            return nil
        }
        defer { IOObjectRelease(iterator) }

        let vendor = CGDisplayVendorNumber(displayID)
        let model = CGDisplayModelNumber(displayID)
        let serial = CGDisplaySerialNumber(displayID)
        var matches: [io_service_t] = []

        while true {
            let service = IOIteratorNext(iterator)
            guard service != 0 else { break }

            // A framebuffer with no attached display — routine during clamshell or
            // hot-plug transitions — yields NULL here; taking it would trap.
            guard let infoDictionary = IODisplayCreateInfoDictionary(
                service,
                IOOptionBits(kIODisplayOnlyPreferredName)
            ) else {
                IOObjectRelease(service)
                continue
            }
            let info = infoDictionary.takeRetainedValue() as NSDictionary
            let serviceVendor = (info[kDisplayVendorID] as? NSNumber)?.uint32Value
            let serviceModel = (info[kDisplayProductID] as? NSNumber)?.uint32Value
            let serviceSerial = (info[kDisplaySerialNumber] as? NSNumber)?.uint32Value

            if serviceVendor == vendor, serviceModel == model, serviceSerial == serial {
                matches.append(service)
            } else {
                IOObjectRelease(service)
            }
        }

        guard matches.count == 1 else {
            AppLog.shared.log(
                .brightness,
                "Intel DDC: \(matches.count) framebuffers matched display \(displayID), need exactly 1"
            )
            matches.forEach { IOObjectRelease($0) }
            return nil
        }
        return matches[0]
    }
}

/// DDC/CI brightness over an Intel Mac's public IOKit framebuffer I2C interfaces.
public final class IntelDDCBrightnessService: BrightnessService {

    private let queue = DispatchQueue(label: "com.prompt-smithy.Moonglow.intel-ddc")
    private let lock = NSLock()
    private let displayID: CGDirectDisplayID
    private var connection: IntelI2CConnection?
    private var pendingBrightness = AppSettings.defaultManualBrightness
    private var pendingWrite: DispatchWorkItem?
    private var isDisposed = false

    public init(displayID: CGDirectDisplayID) {
        self.displayID = displayID
        // The first probe runs through refreshSupportAsync from the view model, off the
        // main thread: opening the connection probes the panel with sleeping I2C traffic.
    }

    public var isSupported: Bool {
        lock.lock()
        defer { lock.unlock() }
        return connection != nil && !isDisposed
    }

    /// Re-opens the I2C connection when hardware is currently down. Opening probes the panel,
    /// so a late-answering display is picked up without relaunching.
    public func refreshSupport() {
        _ = openIfNeeded()
    }

    /// Probes on the serial queue so the calling thread never sleeps on I2C.
    public func refreshSupportAsync(completion: (@Sendable (Bool) -> Void)?) {
        queue.async { [weak self] in
            completion?(self?.openIfNeeded() ?? false)
        }
    }

    /// Opens (and probes) the I2C connection unless hardware is already up. Returns true
    /// when this call newly established support. Safe from any thread.
    private func openIfNeeded() -> Bool {
        lock.lock()
        guard connection == nil, !isDisposed else {
            lock.unlock()
            return false
        }
        lock.unlock()

        guard let opened = IntelI2CConnection.open(displayID: displayID) else {
            AppLog.shared.log(.brightness, "Intel DDC probe failed: panel did not answer")
            return false
        }

        AppLog.shared.log(.brightness, "Intel DDC hardware brightness available")
        lock.lock()
        defer { lock.unlock() }
        guard connection == nil, !isDisposed else {
            opened.close()
            return false
        }
        connection = opened
        return true
    }

    public func setBrightness(percent: Int) {
        lock.lock()
        guard connection != nil, !isDisposed else {
            lock.unlock()
            return
        }

        pendingBrightness = AppSettings.clampBrightness(percent)
        pendingWrite?.cancel()
        let work = DispatchWorkItem { [weak self] in self?.applyPendingBrightness() }
        pendingWrite = work
        lock.unlock()

        queue.asyncAfter(deadline: .now() + DDCPacket.debounceDelay, execute: work)
    }

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

        queue.sync {}

        lock.lock()
        connection?.close()
        connection = nil
        lock.unlock()
    }

    private func applyPendingBrightness() {
        lock.lock()
        let connection = connection
        let percent = pendingBrightness
        let disposed = isDisposed
        lock.unlock()

        guard let connection, !disposed else { return }

        for _ in 0..<DDCPacket.maxAttempts {
            Thread.sleep(forTimeInterval: DDCPacket.writeDelay)
            if connection.writeBrightness(percent: percent) { return }
        }
    }
}
#endif
