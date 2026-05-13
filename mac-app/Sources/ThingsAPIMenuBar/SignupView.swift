import SwiftUI

/// Default control plane URL. Mirrors the `DEFAULT_CONTROL_PLANE` const in src/main.rs.
/// Once you have a deployed control plane, change both in lockstep.
let defaultControlPlaneURL = "https://things-api-control-plane.fly.dev"

struct SignupView: View {
    @ObservedObject var accountStore: AccountStore
    @ObservedObject var serverManager: ServerManager

    @Environment(\.dismiss) private var dismiss

    @State private var username: String = ""
    @State private var email: String = ""
    @State private var controlPlane: String = defaultControlPlaneURL
    @State private var isSubmitting = false
    @State private var errorMessage: String?
    @State private var success: SignupResponse?

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Get a permanent URL")
                .font(.title2).bold()
            Text("Pick a username. Your API will be reachable at \(usernameHostExample).")
                .foregroundStyle(.secondary)

            Form {
                TextField("Username", text: $username)
                    .textFieldStyle(.roundedBorder)
                    .disableAutocorrection(true)
                    .disabled(isSubmitting || success != nil)

                TextField("Email (optional, for recovery)", text: $email)
                    .textFieldStyle(.roundedBorder)
                    .disableAutocorrection(true)
                    .disabled(isSubmitting || success != nil)

                DisclosureGroup("Advanced") {
                    TextField("Control plane URL", text: $controlPlane)
                        .textFieldStyle(.roundedBorder)
                        .disableAutocorrection(true)
                        .disabled(isSubmitting || success != nil)
                }
            }

            if let err = errorMessage {
                Text(err)
                    .font(.callout)
                    .foregroundStyle(.red)
            }

            if let s = success {
                VStack(alignment: .leading, spacing: 4) {
                    Text("✓ Signed up as \(s.username)").bold()
                    Text(s.url).font(.system(.body, design: .monospaced))
                }
                .padding(8)
                .background(Color.green.opacity(0.1))
                .cornerRadius(6)
            }

            HStack {
                Spacer()
                Button("Cancel") { dismiss() }
                    .disabled(isSubmitting)
                Button(success == nil ? "Sign up" : "Done") {
                    if success != nil {
                        dismiss()
                    } else {
                        submit()
                    }
                }
                .keyboardShortcut(.return)
                .disabled(isSubmitting || (success == nil && !canSubmit))
            }
        }
        .padding(20)
        .frame(minWidth: 380)
    }

    private var canSubmit: Bool {
        let u = username.trimmingCharacters(in: .whitespaces).lowercased()
        guard u.count >= 3, u.count <= 32 else { return false }
        return u.allSatisfy { $0.isLetter || $0.isNumber || $0 == "-" }
    }

    private var usernameHostExample: String {
        let host = URL(string: controlPlane)?.host ?? "your-control-plane"
        // Best-effort guess at root domain from the control plane host:
        // "things-api-control-plane.fly.dev" → fallback hint.
        let example = username.isEmpty ? "<username>" : username
        // We don't actually know the root domain client-side; tell the user the control plane decides it.
        _ = host
        return "https://\(example).<your-domain>"
    }

    private func submit() {
        let user = username.trimmingCharacters(in: .whitespaces).lowercased()
        let mail = email.trimmingCharacters(in: .whitespaces)
        let cp = controlPlane.trimmingCharacters(in: .whitespaces)

        guard let cpURL = URL(string: cp), let signupURL = URL(string: "\(cp.trimmedTrailingSlash())/signup") else {
            errorMessage = "Invalid control plane URL"
            return
        }
        _ = cpURL

        isSubmitting = true
        errorMessage = nil

        let body = SignupRequest(username: user, email: mail.isEmpty ? nil : mail)

        Task {
            do {
                var req = URLRequest(url: signupURL)
                req.httpMethod = "POST"
                req.setValue("application/json", forHTTPHeaderField: "Content-Type")
                req.httpBody = try JSONEncoder().encode(body)
                req.timeoutInterval = 30

                let (data, response) = try await URLSession.shared.data(for: req)
                guard let http = response as? HTTPURLResponse else {
                    throw NSError(domain: "Signup", code: 0,
                                  userInfo: [NSLocalizedDescriptionKey: "no HTTP response"])
                }
                if !(200...299).contains(http.statusCode) {
                    let msg = (try? JSONDecoder().decode(ServerError.self, from: data))?.error
                        ?? "HTTP \(http.statusCode)"
                    throw NSError(domain: "Signup", code: http.statusCode,
                                  userInfo: [NSLocalizedDescriptionKey: msg])
                }

                let resp = try JSONDecoder().decode(SignupResponse.self, from: data)

                // Persist account
                let account = Account(
                    username: resp.username,
                    url: resp.url,
                    tunnelToken: resp.tunnelToken,
                    controlPlaneUrl: cp,
                    createdAt: ISO8601DateFormatter().string(from: Date())
                )
                await MainActor.run {
                    do {
                        try accountStore.writeAccount(account)
                        try accountStore.ensureAuthToken()
                        success = resp
                        // Start the server immediately so the URL works right away.
                        if serverManager.state == .stopped {
                            serverManager.start()
                        }
                    } catch {
                        errorMessage = "Persisted signup but failed to save locally: \(error.localizedDescription)"
                    }
                    isSubmitting = false
                }
            } catch {
                await MainActor.run {
                    errorMessage = error.localizedDescription
                    isSubmitting = false
                }
            }
        }
    }
}

private extension String {
    func trimmedTrailingSlash() -> String {
        var s = self
        while s.hasSuffix("/") { s.removeLast() }
        return s
    }
}
