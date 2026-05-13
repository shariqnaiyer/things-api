import Foundation
import os.log

enum ServerState: Equatable {
    case stopped
    case starting
    case running
    case error(String)
}

/// Owns the things-api child process. Spawns it, watches its stdout/stderr, and polls /health
/// over localhost to decide whether to surface as running/error.
@MainActor
final class ServerManager: ObservableObject {
    @Published private(set) var state: ServerState = .stopped
    @Published private(set) var lastHealthCheck: Date?
    @Published private(set) var recentLog: [String] = []

    /// Local port the server binds. Mirrors the Rust default.
    var port: Int = 3333

    /// Override for the things-api binary path. If nil, look it up next to this app's executable.
    var binaryOverride: URL?

    private var process: Process?
    private var stdoutPipe: Pipe?
    private var stderrPipe: Pipe?
    private var healthTask: Task<Void, Never>?

    private let logger = Logger(subsystem: "io.anywhere-api.ThingsAPIMenuBar", category: "server")

    private weak var accountStore: AccountStore?

    init(accountStore: AccountStore) {
        self.accountStore = accountStore
    }

    // MARK: - Lifecycle

    func start() {
        guard process == nil else { return }
        guard let binary = resolveBinary() else {
            state = .error("things-api binary not found")
            return
        }

        state = .starting
        recentLog.removeAll()

        let p = Process()
        p.executableURL = binary
        p.arguments = ["run"]

        // Pass through any env the user might have set, but inherit by default.
        var env = ProcessInfo.processInfo.environment
        // Ensure the binary writes its token/account to the same place we read from.
        env.removeValue(forKey: "THINGS_AUTH_TOKEN")
        p.environment = env

        let stdout = Pipe()
        let stderr = Pipe()
        p.standardOutput = stdout
        p.standardError = stderr
        self.stdoutPipe = stdout
        self.stderrPipe = stderr

        captureOutput(stdout, label: "stdout")
        captureOutput(stderr, label: "stderr")

        p.terminationHandler = { [weak self] _ in
            Task { @MainActor in
                guard let self else { return }
                let wasRunning = self.state == .running || self.state == .starting
                self.process = nil
                self.stdoutPipe = nil
                self.stderrPipe = nil
                self.healthTask?.cancel()
                self.healthTask = nil
                // If we expected to be running, surface as error so the user sees the icon turn red.
                if wasRunning {
                    self.state = .error("server exited unexpectedly")
                } else {
                    self.state = .stopped
                }
            }
        }

        do {
            try p.run()
            self.process = p
            startHealthPolling()
        } catch {
            state = .error("failed to launch: \(error.localizedDescription)")
            self.process = nil
        }
    }

    func stop() {
        healthTask?.cancel()
        healthTask = nil
        guard let p = process else {
            state = .stopped
            return
        }
        // SIGTERM first — gives axum a chance to drain. terminationHandler will fire and reset state.
        p.terminate()
        // Belt-and-suspenders: kill after 3s if still alive.
        Task { @MainActor in
            try? await Task.sleep(nanoseconds: 3_000_000_000)
            if let p = self.process, p.isRunning {
                kill(p.processIdentifier, SIGKILL)
            }
        }
    }

    func restart() {
        stop()
        Task { @MainActor in
            // Wait for the termination handler to fire before launching again.
            try? await Task.sleep(nanoseconds: 500_000_000)
            self.start()
        }
    }

    // MARK: - Binary resolution

    /// In a built .app, the things-api binary lives at Contents/MacOS/things-api alongside us.
    /// In `swift run` dev mode, we fall back to ../target/release/things-api.
    private func resolveBinary() -> URL? {
        if let override = binaryOverride {
            return FileManager.default.fileExists(atPath: override.path) ? override : nil
        }

        let here = Bundle.main.executableURL?.deletingLastPathComponent()
            ?? URL(fileURLWithPath: CommandLine.arguments[0]).deletingLastPathComponent()
        let bundled = here.appendingPathComponent("things-api")
        if FileManager.default.fileExists(atPath: bundled.path) {
            return bundled
        }

        // Dev fallback: look at repo target/release/.
        let devCandidates = [
            URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
                .appendingPathComponent("../target/release/things-api"),
            URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
                .appendingPathComponent("target/release/things-api"),
        ]
        return devCandidates.first { FileManager.default.fileExists(atPath: $0.path) }
    }

    // MARK: - Output capture

    private func captureOutput(_ pipe: Pipe, label: String) {
        pipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty, let text = String(data: data, encoding: .utf8) else { return }
            Task { @MainActor in
                guard let self else { return }
                for line in text.split(separator: "\n", omittingEmptySubsequences: true) {
                    let entry = "[\(label)] \(line)"
                    self.recentLog.append(entry)
                    if self.recentLog.count > 200 {
                        self.recentLog.removeFirst(self.recentLog.count - 200)
                    }
                }
            }
        }
    }

    // MARK: - Health polling

    private func startHealthPolling() {
        healthTask?.cancel()
        healthTask = Task { @MainActor [weak self] in
            guard let self else { return }
            // Phase 1: probe every 250ms for up to 10s waiting for the server to come up.
            let startupDeadline = Date().addingTimeInterval(10)
            while !Task.isCancelled, Date() < startupDeadline {
                if await self.probeHealth() {
                    self.state = .running
                    self.lastHealthCheck = Date()
                    break
                }
                try? await Task.sleep(nanoseconds: 250_000_000)
            }
            if Task.isCancelled { return }
            if self.state != .running {
                self.state = .error("server did not respond to /health within 10s")
                return
            }
            // Phase 2: steady-state probe every 5s. Surface errors but don't kill the process —
            // cloudflared may be reconnecting; let it recover.
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 5_000_000_000)
                if Task.isCancelled { return }
                let ok = await self.probeHealth()
                if ok {
                    self.state = .running
                    self.lastHealthCheck = Date()
                } else if case .running = self.state {
                    self.state = .error("/health stopped responding")
                }
            }
        }
    }

    private func probeHealth() async -> Bool {
        guard let url = URL(string: "http://127.0.0.1:\(port)/health") else { return false }
        var req = URLRequest(url: url)
        req.timeoutInterval = 2
        do {
            let (_, response) = try await URLSession.shared.data(for: req)
            return (response as? HTTPURLResponse)?.statusCode == 200
        } catch {
            return false
        }
    }
}
