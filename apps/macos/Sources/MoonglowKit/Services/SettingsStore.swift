import Foundation

/// Loads and persists ``AppSettings`` as JSON.
///
/// Ported from `Services/SettingsService.cs`, minus the legacy import path. This is a
/// pure persistence adapter: the observable copy of settings is owned by the view model.
/// Slider writes are coalesced on a serial queue and flushed before quit.
/// Filesystem failures are best effort and never crash the app.
public final class SettingsStore {

    /// `~/Library/Application Support/Moonglow/settings.json`
    public static func defaultFileURL(fileManager: FileManager = .default) -> URL {
        let base = fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? fileManager.homeDirectoryForCurrentUser
                .appendingPathComponent("Library/Application Support", isDirectory: true)
        return base
            .appendingPathComponent("Moonglow", isDirectory: true)
            .appendingPathComponent("settings.json")
    }

    public let fileURL: URL

    private let fileManager: FileManager
    private let writeQueue = DispatchQueue(label: "com.prompt-smithy.Moonglow.settings")
    private var pendingSettings: AppSettings?
    private var pendingWrite: DispatchWorkItem?

    public init(fileURL: URL = SettingsStore.defaultFileURL(), fileManager: FileManager = .default) {
        self.fileURL = fileURL
        self.fileManager = fileManager
    }

    /// Reads settings from disk and normalizes them. A missing file falls back to
    /// defaults; an unreadable or corrupt one does the same, but is first moved aside to
    /// `settings.json.corrupt` so the next save does not silently destroy it.
    public func load() -> AppSettings {
        flush()
        guard fileManager.fileExists(atPath: fileURL.path) else { return .default }
        guard let data = try? Data(contentsOf: fileURL),
              let decoded = try? JSONDecoder().decode(AppSettings.self, from: data)
        else {
            AppLog.shared.log(
                .settings,
                "settings file unreadable, falling back to defaults; keeping the original as \(corruptFileURL.lastPathComponent)"
            )
            preserveCorruptFile()
            return .default
        }

        return decoded.normalized()
    }

    /// Moves an unreadable settings file aside, replacing any previous preservation.
    /// Best effort, like everything else here.
    private func preserveCorruptFile() {
        try? fileManager.removeItem(at: corruptFileURL)
        try? fileManager.moveItem(at: fileURL, to: corruptFileURL)
    }

    private var corruptFileURL: URL { fileURL.appendingPathExtension("corrupt") }

    /// Persists settings, normalizing them first so out-of-range values never reach disk.
    ///
    /// Writes first and creates the directory only when the write fails for want of it,
    /// so the slider hot path pays for one atomic write instead of a mkdir on every tick.
    public func save(_ settings: AppSettings) {
        writeQueue.sync {
            pendingWrite?.cancel()
            pendingWrite = nil
            pendingSettings = nil
            write(settings)
        }
    }

    /// Coalesces slider updates without doing filesystem work on the main thread.
    public func scheduleSave(_ settings: AppSettings) {
        writeQueue.async { [self] in
            pendingSettings = settings
            pendingWrite?.cancel()
            let work = DispatchWorkItem { [self] in writePending() }
            pendingWrite = work
            writeQueue.asyncAfter(deadline: .now() + 0.25, execute: work)
        }
    }

    /// Commits the most recent snapshot before quit or an explicit read.
    public func flush() {
        writeQueue.sync { writePending() }
    }

    private func writePending() {
        pendingWrite?.cancel()
        pendingWrite = nil
        guard let settings = pendingSettings else { return }
        pendingSettings = nil
        write(settings)
    }

    private func write(_ settings: AppSettings) {
        do {
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            let data = try encoder.encode(settings.normalized())
            do {
                try data.write(to: fileURL, options: .atomic)
            } catch {
                try fileManager.createDirectory(
                    at: fileURL.deletingLastPathComponent(),
                    withIntermediateDirectories: true
                )
                try data.write(to: fileURL, options: .atomic)
            }
        } catch {
            AppLog.shared.log(.settings, "settings save failed: \(error.localizedDescription)")
        }
    }
}
