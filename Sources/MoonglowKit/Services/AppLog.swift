import Foundation

/// What part of the app a log line describes.
public enum AppLogCategory: String, Sendable {
    case lifecycle
    case brightness
    case display
    case settings
    case schedule
    case loginItem
}

/// Append-only rotating file log for post-hoc diagnosis.
///
/// The app makes silent hardware decisions — DDC probe outcomes, fallback routing, per-display
/// capabilities — that are invisible once the moment passes. This log keeps them on disk so a
/// recurrence can be diagnosed from `~/Library/Logs/Moonglow/` instead of reproduced.
///
/// Writes are small, synchronous, lock-guarded, and best effort: a filesystem failure must never
/// crash or stall the app. Rotation keeps the current file plus `maxFiles` older ones, bounding
/// total history to roughly `maxBytes * (maxFiles + 1)`.
///
/// `shared` stays inactive until `start()` so unit tests never touch the real log; instances
/// created directly (as tests do) are active immediately.
public final class AppLog: @unchecked Sendable {

    public static let shared = AppLog(active: false)

    /// `~/Library/Logs/Moonglow/Moonglow.log`
    public static func defaultFileURL(fileManager: FileManager = .default) -> URL {
        let base = fileManager.urls(for: .libraryDirectory, in: .userDomainMask).first
            ?? fileManager.homeDirectoryForCurrentUser
                .appendingPathComponent("Library", isDirectory: true)
        return base
            .appendingPathComponent("Logs", isDirectory: true)
            .appendingPathComponent("Moonglow", isDirectory: true)
            .appendingPathComponent("Moonglow.log")
    }

    public let fileURL: URL

    private let fileManager: FileManager
    private let maxBytes: Int
    private let maxFiles: Int
    private let lock = NSLock()
    private var isActive: Bool

    /// Set once the directory is known to exist, so the slider hot path pays for one
    /// write per line instead of create-directory + stat + open + write + close.
    private var directoryReady = false

    /// Held open across appends; closed around rotation and reopened lazily.
    private var fileHandle: FileHandle?

    /// Bytes in the current file, tracked across appends so rotation needs no stat.
    private var knownSize = 0

    /// Only touched under `lock`.
    private let stamp: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter
    }()

    public init(
        fileURL: URL = AppLog.defaultFileURL(),
        fileManager: FileManager = .default,
        maxBytes: Int = 512 * 1024,
        maxFiles: Int = 3,
        active: Bool = true
    ) {
        self.fileURL = fileURL
        self.fileManager = fileManager
        self.maxBytes = maxBytes
        self.maxFiles = maxFiles
        self.isActive = active
    }

    /// Activates the log. Called once at launch; idempotent.
    public func start() {
        lock.lock()
        defer { lock.unlock() }
        isActive = true
    }

    /// Appends one timestamped line. The message is only built when the log is active.
    public func log(_ category: AppLogCategory, _ message: @autoclosure () -> String) {
        lock.lock()
        defer { lock.unlock() }

        guard isActive else { return }

        append("\(stamp.string(from: Date())) [\(category.rawValue)] \(message())\n")
    }

    private func append(_ line: String) {
        do {
            if !directoryReady {
                try fileManager.createDirectory(
                    at: fileURL.deletingLastPathComponent(),
                    withIntermediateDirectories: true
                )
                directoryReady = true
            }
            guard let data = line.data(using: .utf8) else { return }
            if maxFiles > 0, knownSize >= maxBytes {
                rotate()
            }
            try openHandle().write(contentsOf: data)
            knownSize += data.count
        } catch {
            directoryReady = false
            closeHandle()
            knownSize = 0
        }
    }

    /// Opens the log once and seeks to the end, learning the pre-existing size (an
    /// earlier run's lines) without a separate stat.
    private func openHandle() throws -> FileHandle {
        if let fileHandle { return fileHandle }
        if !fileManager.fileExists(atPath: fileURL.path) {
            fileManager.createFile(atPath: fileURL.path, contents: nil)
        }
        let handle = try FileHandle(forWritingTo: fileURL)
        knownSize = Int(try handle.seekToEnd())
        fileHandle = handle
        return handle
    }

    private func closeHandle() {
        try? fileHandle?.close()
        fileHandle = nil
    }

    private func rotate() {
        // The handle must go first: renaming the file out from under it would send
        // later lines to the rotated-away inode.
        closeHandle()

        try? fileManager.removeItem(at: rotatedURL(maxFiles))
        for index in stride(from: maxFiles - 1, through: 1, by: -1) {
            let source = rotatedURL(index)
            if fileManager.fileExists(atPath: source.path) {
                try? fileManager.moveItem(at: source, to: rotatedURL(index + 1))
            }
        }
        try? fileManager.moveItem(at: fileURL, to: rotatedURL(1))
        knownSize = 0
    }

    private func rotatedURL(_ index: Int) -> URL {
        fileURL.appendingPathExtension("\(index)")
    }
}
