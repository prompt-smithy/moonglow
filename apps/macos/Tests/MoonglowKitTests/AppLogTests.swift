import Foundation
import Testing
@testable import MoonglowKit

@Suite("App log")
struct AppLogTests {

    @Test("Lines carry a timestamp, a category, and the message")
    func linesCarryTimestampCategoryAndMessage() {
        let log = AppLog(fileURL: tempURL())

        log.log(.brightness, "probe answered")

        let contents = read(log.fileURL)
        #expect(contents.range(of: #"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}"#, options: .regularExpression) != nil)
        #expect(contents.contains("[brightness] probe answered"))
        #expect(contents.hasSuffix("\n"))
    }

    @Test("An inactive log writes nothing until started")
    func inactiveLogWritesNothing() {
        let log = AppLog(fileURL: tempURL(), active: false)

        log.log(.brightness, "silent")

        #expect(!FileManager.default.fileExists(atPath: log.fileURL.path))

        log.start()
        log.log(.brightness, "audible")

        #expect(read(log.fileURL).contains("audible"))
    }

    @Test("An oversized log rotates, keeping bounded history")
    func oversizedLogRotates() {
        let log = AppLog(fileURL: tempURL(), maxBytes: 120, maxFiles: 2)

        for index in 0..<30 {
            log.log(.display, "enumeration \(index) with padding to overflow quickly")
        }

        #expect(FileManager.default.fileExists(atPath: log.fileURL.path))
        #expect(FileManager.default.fileExists(atPath: log.fileURL.appendingPathExtension("1").path))
        #expect(!FileManager.default.fileExists(atPath: log.fileURL.appendingPathExtension("3").path))
    }

    @Test("The default location is the shared Moonglow log folder")
    func defaultLocationIsSharedLogFolder() {
        #expect(AppLog.defaultFileURL().path.hasSuffix("Library/Logs/Moonglow/Moonglow.log"))
    }
}

private func tempURL() -> URL {
    FileManager.default.temporaryDirectory
        .appendingPathComponent("applog-\(UUID().uuidString)", isDirectory: true)
        .appendingPathComponent("test.log")
}

private func read(_ url: URL) -> String {
    (try? String(contentsOf: url, encoding: .utf8)) ?? ""
}
