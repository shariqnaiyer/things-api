import SwiftUI

@main
struct ThingsAPIMenuBarApp: App {
    @StateObject private var accountStore = AccountStore()
    @StateObject private var serverManager: ServerManager
    @State private var showSignup = false
    @State private var showSettings = false

    init() {
        let store = AccountStore()
        let manager = ServerManager(accountStore: store)
        // Auto-start at launch if the user is already signed up. This is what makes
        // "Launch at Login" actually serve traffic without a click.
        if store.account != nil {
            manager.start()
        }
        _accountStore = StateObject(wrappedValue: store)
        _serverManager = StateObject(wrappedValue: manager)
    }

    var body: some Scene {
        MenuBarExtra {
            MenuView(
                serverManager: serverManager,
                accountStore: accountStore,
                showSignup: $showSignup,
                showSettings: $showSettings
            )
        } label: {
            Image(systemName: iconName(for: serverManager.state))
        }
        .menuBarExtraStyle(.menu)

        // Standalone windows summoned by the menu.
        Window("Sign up · things-api", id: "signup") {
            SignupView(accountStore: accountStore, serverManager: serverManager)
                .frame(minWidth: 360, minHeight: 280)
        }
        .windowResizability(.contentSize)

        Window("Settings · things-api", id: "settings") {
            SettingsView(accountStore: accountStore, serverManager: serverManager)
                .frame(minWidth: 420, minHeight: 320)
        }
        .windowResizability(.contentSize)
    }

    private func iconName(for state: ServerState) -> String {
        switch state {
        case .stopped:
            return "circle"
        case .starting:
            return "arrow.triangle.2.circlepath"
        case .running:
            return "checkmark.circle.fill"
        case .error:
            return "exclamationmark.triangle.fill"
        }
    }
}
