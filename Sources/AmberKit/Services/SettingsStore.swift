import Foundation

/// Loads and persists ``AppSettings`` as JSON.
///
/// Ported from `Services/SettingsService.cs`, minus the legacy import path. This is a
/// pure persistence adapter: the observable copy of settings is owned by the view model.
/// Writes are small, synchronous, and best effort — a filesystem failure must never
/// crash the app.
public final class SettingsStore {

    /// `~/Library/Application Support/Amber/settings.json`
    public static func defaultFileURL(fileManager: FileManager = .default) -> URL {
        let base = fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? fileManager.homeDirectoryForCurrentUser
                .appendingPathComponent("Library/Application Support", isDirectory: true)
        return base
            .appendingPathComponent("Amber", isDirectory: true)
            .appendingPathComponent("settings.json")
    }

    public let fileURL: URL

    private let fileManager: FileManager

    public init(fileURL: URL = SettingsStore.defaultFileURL(), fileManager: FileManager = .default) {
        self.fileURL = fileURL
        self.fileManager = fileManager
    }

    /// Reads settings from disk and normalizes them. A missing, unreadable, or corrupt
    /// file falls back to defaults.
    public func load() -> AppSettings {
        guard let data = try? Data(contentsOf: fileURL),
              let decoded = try? JSONDecoder().decode(AppSettings.self, from: data)
        else {
            return .default
        }

        return decoded.normalized()
    }

    /// Persists settings, normalizing them first so out-of-range values never reach disk.
    public func save(_ settings: AppSettings) {
        do {
            try fileManager.createDirectory(
                at: fileURL.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            try encoder.encode(settings.normalized()).write(to: fileURL, options: .atomic)
        } catch {
            // Best effort by design.
        }
    }
}
