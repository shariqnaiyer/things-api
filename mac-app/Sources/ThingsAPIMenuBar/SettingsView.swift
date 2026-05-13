import SwiftUI
import ServiceManagement

struct SettingsView: View {
    @ObservedObject var accountStore: AccountStore
    @ObservedObject var serverManager: ServerManager

    @Environment(\.dismiss) private var dismiss

    @State private var port: String = "3333"
    @State private var launchAtLogin: Bool = false
    @State private var launchAtLoginError: String?
    @State private var showResetConfirm = false

    var body: some View {
        Form {
            Section {
                LabeledContent("Port") {
                    TextField("3333", text: $port)
                        .frame(width: 80)
                        .textFieldStyle(.roundedBorder)
                        .onSubmit { applyPort() }
                }
                Toggle("Launch at Login", isOn: $launchAtLogin)
                    .onChange(of: launchAtLogin) { newValue in
                        updateLaunchAtLogin(to: newValue)
                    }
                if let err = launchAtLoginError {
                    Text(err).font(.caption).foregroundStyle(.red)
                }
            } header: {
                Text("General")
            }

            Section {
                if let account = accountStore.account {
                    LabeledContent("Username", value: account.username)
                    LabeledContent("URL") {
                        Text(account.url)
                            .font(.system(.body, design: .monospaced))
                            .textSelection(.enabled)
                    }
                    LabeledContent("Control plane", value: account.controlPlaneUrl)
                    LabeledContent("Signed up", value: account.createdAt)
                } else {
                    Text("Not signed up yet — pick \"Sign up\" from the menu.")
                        .foregroundStyle(.secondary)
                }
            } header: {
                Text("Account")
            }

            Section {
                if let token = accountStore.authToken {
                    LabeledContent("Bearer") {
                        Text(token)
                            .font(.system(.callout, design: .monospaced))
                            .textSelection(.enabled)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                } else {
                    Text("No token yet").foregroundStyle(.secondary)
                }
                HStack {
                    Button("Rotate token") {
                        do {
                            _ = try accountStore.rotateToken()
                            if case .running = serverManager.state {
                                serverManager.restart()
                            }
                        } catch {
                            NSLog("rotate: \(error)")
                        }
                    }
                    Button("Reset account", role: .destructive) {
                        showResetConfirm = true
                    }
                    .disabled(accountStore.account == nil)
                }
            } header: {
                Text("Authentication")
            }

            Section {
                LabeledContent("Config directory") {
                    Text(AccountStore.configDir.path)
                        .font(.system(.caption, design: .monospaced))
                        .textSelection(.enabled)
                }
                Button("Reveal in Finder") {
                    NSWorkspace.shared.activateFileViewerSelecting([AccountStore.configDir])
                }
            } header: {
                Text("Files")
            }
        }
        .formStyle(.grouped)
        .padding()
        .frame(minWidth: 440, minHeight: 460)
        .onAppear {
            port = String(serverManager.port)
            launchAtLogin = SMAppService.mainApp.status == .enabled
        }
        .confirmationDialog(
            "Remove the signed-up account from this Mac?",
            isPresented: $showResetConfirm,
            titleVisibility: .visible
        ) {
            Button("Remove account", role: .destructive) {
                accountStore.clearAccount()
                serverManager.stop()
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This deletes account.json locally. Your subdomain remains reserved on the control plane.")
        }
    }

    private func applyPort() {
        if let p = Int(port), (1...65535).contains(p), p != serverManager.port {
            serverManager.port = p
            if case .running = serverManager.state {
                serverManager.restart()
            }
        }
    }

    private func updateLaunchAtLogin(to enabled: Bool) {
        do {
            if enabled {
                if SMAppService.mainApp.status != .enabled {
                    try SMAppService.mainApp.register()
                }
            } else {
                if SMAppService.mainApp.status == .enabled {
                    try SMAppService.mainApp.unregister()
                }
            }
            launchAtLoginError = nil
        } catch {
            launchAtLoginError = "Couldn't update: \(error.localizedDescription)"
            // Revert toggle state to reality.
            launchAtLogin = SMAppService.mainApp.status == .enabled
        }
    }
}
