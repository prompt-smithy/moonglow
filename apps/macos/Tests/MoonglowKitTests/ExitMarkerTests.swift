import Foundation
import Testing
@testable import MoonglowKit

@Suite("Exit marker")
struct ExitMarkerTests {

    private static func makeMarker() -> ExitMarker {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("moonglow-exit-\(UUID().uuidString)", isDirectory: true)
        return ExitMarker(fileURL: directory.appendingPathComponent("running.marker"))
    }

    @Test("A first launch finds no marker and leaves one behind")
    func firstLaunchMarks() {
        let marker = Self.makeMarker()

        #expect(marker.enter() == false)
        #expect(FileManager.default.fileExists(atPath: marker.fileURL.path))
    }

    @Test("A marker left behind reads as an unclean exit")
    func leftoverMarkerReadsUnclean() {
        let marker = Self.makeMarker()
        marker.enter()

        #expect(ExitMarker(fileURL: marker.fileURL).enter() == true)
    }

    @Test("A clean exit clears the marker")
    func cleanExitClears() {
        let marker = Self.makeMarker()
        marker.enter()

        marker.exitCleanly()

        #expect(!FileManager.default.fileExists(atPath: marker.fileURL.path))
        #expect(marker.enter() == false)
    }

    @Test("Each copy gets its own marker file")
    func markersArePerInstance() {
        let packaged = ExitMarker.defaultFileURL(instanceIdentifier: "com.prompt-smithy.Moonglow")
        let unpackaged = ExitMarker.defaultFileURL(instanceIdentifier: "unpackaged")

        #expect(packaged != unpackaged)
        #expect(packaged.lastPathComponent == "running-com.prompt-smithy.Moonglow.marker")
        #expect(unpackaged.lastPathComponent == "running-unpackaged.marker")
    }

    @Test("An identifier cannot escape the marker directory")
    func identifierCannotEscapeDirectory() {
        let url = ExitMarker.defaultFileURL(instanceIdentifier: "com/evil")

        #expect(url.lastPathComponent == "running-com-evil.marker")
        #expect(url.deletingLastPathComponent().lastPathComponent == "Moonglow")
    }
}
