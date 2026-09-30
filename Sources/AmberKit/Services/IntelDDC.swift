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
            replyByteCount: 11
        ), response.count == 11 else {
            return false
        }

        let expectedChecksum = response.dropLast().reduce(UInt8(0x50)) { $0 ^ $1 }
        return response[2] == 0x02
            && response[3] == 0x00
            && response[4] == 0x10
            && response[10] == expectedChecksum
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

            let info = IODisplayCreateInfoDictionary(
                service,
                IOOptionBits(kIODisplayOnlyPreferredName)
            ).takeRetainedValue() as NSDictionary
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
            matches.forEach { IOObjectRelease($0) }
            return nil
        }
        return matches[0]
    }
}

/// DDC/CI brightness over an Intel Mac's public IOKit framebuffer I2C interfaces.
public final class IntelDDCBrightnessService: BrightnessService {

    private let queue = DispatchQueue(label: "com.amber.intel-ddc")
    private let lock = NSLock()
    private var connection: IntelI2CConnection?
    private var pendingBrightness = AppSettings.defaultManualBrightness
    private var pendingWrite: DispatchWorkItem?
    private var isDisposed = false

    public init(displayID: CGDirectDisplayID) {
        connection = IntelI2CConnection.open(displayID: displayID)
    }

    public var isSupported: Bool {
        lock.lock()
        defer { lock.unlock() }
        return connection != nil && !isDisposed
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