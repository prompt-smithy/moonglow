import Foundation

/// Remembers which packaged app installation has already completed its first-launch setup.
public final class InstallationStore {

    private let defaults: UserDefaults
    private let key = "com.amber.macos.installationID"

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    public func hasSeen(identifier: String) -> Bool {
        guard !identifier.isEmpty else { return true }
        return defaults.string(forKey: key) == identifier
    }

    public func markSeen(identifier: String) {
        guard !identifier.isEmpty else { return }
        defaults.set(identifier, forKey: key)
    }
}