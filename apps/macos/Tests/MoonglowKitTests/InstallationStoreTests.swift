import Foundation
import Testing
@testable import MoonglowKit

@Suite("Installation identity")
struct InstallationStoreTests {

    @Test("A new package identity is recognized once")
    func packageIdentityIsRemembered() {
        let suiteName = "InstallationStoreTests.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suiteName)!
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let store = InstallationStore(defaults: defaults)

        #expect(!store.hasSeen(identifier: "install-one"))
        store.markSeen(identifier: "install-one")
        #expect(store.hasSeen(identifier: "install-one"))
        #expect(!store.hasSeen(identifier: "install-two"))
    }
}
