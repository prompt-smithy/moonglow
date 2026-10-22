import Foundation

/// Detects an abnormal exit so the next launch can undo leftover display state.
///
/// A marker file is created at launch and removed on graceful quit. If the marker is
/// already present at launch, the previous run died — crash, kill, force-quit — with
/// gamma ramps possibly still applied, and the caller restores ColorSync state before
/// doing anything else.
///
/// The marker is per-instance (see ``defaultFileURL(instanceIdentifier:fileManager:)``),
/// so two copies of Moonglow on one machine cannot mistake each other for a crashed run.
public final class ExitMarker {

    /// `~/Library/Application Support/Moonglow/running-<identifier>.marker`
    ///
    /// The identifier keeps copies apart. A packaged `Moonglow.app` and an unpackaged
    /// `swift run` share the support directory, and with a single marker file each would
    /// read the other's as a crashed run and restore ColorSync gamma state out from under
    /// a copy that is still running — which, with the ramp cache, is not repaired.
    ///
    /// Path separators are replaced so the identifier cannot escape the directory.
    public static func defaultFileURL(
        instanceIdentifier: String = "default",
        fileManager: FileManager = .default
    ) -> URL {
        let base = fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? fileManager.homeDirectoryForCurrentUser
                .appendingPathComponent("Library/Application Support", isDirectory: true)
        return base
            .appendingPathComponent("Moonglow", isDirectory: true)
            .appendingPathComponent(
                "running-\(instanceIdentifier.replacingOccurrences(of: "/", with: "-")).marker"
            )
    }

    public let fileURL: URL

    private let fileManager: FileManager

    public init(fileURL: URL = ExitMarker.defaultFileURL(), fileManager: FileManager = .default) {
        self.fileURL = fileURL
        self.fileManager = fileManager
    }

    /// Creates the marker. Returns true when one was already present, meaning the
    /// previous run did not exit cleanly.
    @discardableResult
    public func enter() -> Bool {
        let unclean = fileManager.fileExists(atPath: fileURL.path)
        try? fileManager.createDirectory(
            at: fileURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        fileManager.createFile(atPath: fileURL.path, contents: nil)
        return unclean
    }

    /// Removes the marker. Called last on graceful quit; anything that prevents this
    /// — crash, kill, a hang in teardown — correctly reads as unclean next time.
    public func exitCleanly() {
        try? fileManager.removeItem(at: fileURL)
    }
}
