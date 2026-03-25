mod applescript;
mod models;
mod routes;

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::{self, Next},
    response::IntoResponse,
    routing::{get, patch},
    Json, Router,
};
use clap::Parser;
use rand::Rng;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use models::HealthResponse;
use routes::{
    projects::list_projects,
    tags::{list_areas, list_tags},
    tasks::{complete_task, create_task, delete_task, get_task, list_tasks, update_task},
};

#[derive(Parser)]
#[command(name = "things-api", about = "REST API server for Things 3")]
struct Args {
    /// Expose the server via a Cloudflare HTTPS tunnel (no account needed)
    #[arg(long)]
    tunnel: bool,

    /// Port to listen on (default: 3333, or PORT env var)
    #[arg(short, long)]
    port: Option<u16>,
}

async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }),
    )
}

fn generate_token() -> String {
    let bytes: [u8; 24] = rand::rng().random();
    let encoded = bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();
    format!("thingsapi_{}", encoded)
}

fn resolve_token() -> String {
    std::env::var("THINGS_AUTH_TOKEN").unwrap_or_else(|_| generate_token())
}

async fn auth_middleware(
    request: Request,
    next: Next,
) -> Result<impl IntoResponse, StatusCode> {
    let token = request
        .extensions()
        .get::<Arc<String>>()
        .expect("auth token not in extensions");

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header == format!("Bearer {}", token) => Ok(next.run(request).await),
        Some(_) => Err(StatusCode::UNAUTHORIZED),
        None => Err(StatusCode::UNAUTHORIZED),
    }
}

fn router(auth_token: Option<String>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_routes = Router::new()
        .route("/tasks", get(list_tasks).post(create_task))
        .route(
            "/tasks/{id}",
            get(get_task).patch(update_task).delete(delete_task),
        )
        .route("/tasks/{id}/complete", patch(complete_task))
        .route("/projects", get(list_projects))
        .route("/tags", get(list_tags))
        .route("/areas", get(list_areas));

    let api_routes = if let Some(token) = auth_token {
        let token = Arc::new(token);
        api_routes
            .layer(middleware::from_fn(auth_middleware))
            .layer(axum::Extension(token))
    } else {
        api_routes
    };

    Router::new()
        .route("/health", get(health))
        .merge(api_routes)
        .layer(cors)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "things_api=debug,tower_http=debug".parse().unwrap()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    let port: u16 = args
        .port
        .or_else(|| std::env::var("PORT").ok().and_then(|p| p.parse().ok()))
        .unwrap_or(3333);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    info!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind TCP listener");

    if args.tunnel {
        let token = resolve_token();
        let tunnel = start_tunnel(port).await;

        println!();
        println!("  HTTPS tunnel: {}", tunnel.url);
        println!("  Auth token:   {}", token);
        println!();
        println!("  Use this header on other devices:");
        println!("  Authorization: Bearer {}", token);
        println!();

        tokio::select! {
            result = axum::serve(listener, router(Some(token))) => {
                result.expect("Server failed");
            }
            _ = tunnel.wait() => {
                eprintln!("cloudflared process exited unexpectedly");
            }
        }
    } else {
        axum::serve(listener, router(None))
            .await
            .expect("Server failed");
    }
}

struct Tunnel {
    url: String,
    child: tokio::process::Child,
}

impl Tunnel {
    async fn wait(mut self) {
        let _ = self.child.wait().await;
    }
}

fn cloudflared_path() -> std::path::PathBuf {
    dirs_for_download().join("cloudflared")
}

fn dirs_for_download() -> std::path::PathBuf {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("things-api");
    std::fs::create_dir_all(&dir).ok();
    dir
}

async fn ensure_cloudflared() -> std::path::PathBuf {
    // Check if cloudflared is on PATH first
    if let Ok(output) = tokio::process::Command::new("which")
        .arg("cloudflared")
        .output()
        .await
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            info!("Found cloudflared at {}", path);
            return std::path::PathBuf::from(path);
        }
    }

    let path = cloudflared_path();
    if path.exists() {
        info!("Using cached cloudflared at {}", path.display());
        return path;
    }

    println!("Downloading cloudflared...");

    let dir = dirs_for_download();

    #[cfg(target_os = "macos")]
    {
        let tgz_path = dir.join("cloudflared.tgz");
        let url = download_url();
        run_curl(&tgz_path, url).await;

        let output = tokio::process::Command::new("tar")
            .args(["-xzf"])
            .arg(&tgz_path)
            .arg("-C")
            .arg(&dir)
            .output()
            .await
            .expect("Failed to extract cloudflared");

        if !output.status.success() {
            panic!(
                "Failed to extract cloudflared: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        std::fs::remove_file(&tgz_path).ok();
    }

    #[cfg(not(target_os = "macos"))]
    {
        let url = download_url();
        run_curl(&path, url).await;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("Failed to make cloudflared executable");
    }

    info!("Downloaded cloudflared to {}", path.display());
    path
}

async fn run_curl(dest: &std::path::Path, url: &str) {
    let output = tokio::process::Command::new("curl")
        .args(["-L", "-o"])
        .arg(dest)
        .arg(url)
        .output()
        .await
        .expect("Failed to run curl — is curl installed?");

    if !output.status.success() {
        panic!(
            "Failed to download cloudflared: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn download_url() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-darwin-arm64.tgz";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-darwin-amd64.tgz";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-arm64";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64";
}

async fn start_tunnel(port: u16) -> Tunnel {
    let cloudflared = ensure_cloudflared().await;

    let mut child = tokio::process::Command::new(&cloudflared)
        .args(["tunnel", "--url", &format!("http://localhost:{}", port)])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to start cloudflared");

    // cloudflared prints the URL to stderr
    let stderr = child.stderr.take().expect("Failed to capture stderr");
    let url = parse_tunnel_url(stderr).await;

    // Re-pipe stderr to our stderr for logging
    Tunnel { url, child }
}

async fn parse_tunnel_url(stderr: tokio::process::ChildStderr) -> String {
    use tokio::io::{AsyncBufReadExt, BufReader};

    let reader = BufReader::new(stderr);
    let mut lines = reader.lines();

    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(30);

    while let Ok(Some(line)) = lines.next_line().await {
        eprintln!("[cloudflared] {}", line);
        if let Some(url_start) = line.find("https://") {
            let url = &line[url_start..];
            let url = url.split_whitespace().next().unwrap_or(url);
            if url.contains(".trycloudflare.com") {
                let found = url.to_string();
                // Keep draining stderr in the background so cloudflared doesn't get a broken pipe
                tokio::spawn(async move {
                    while let Ok(Some(line)) = lines.next_line().await {
                        eprintln!("[cloudflared] {}", line);
                    }
                });
                return found;
            }
        }
        if start.elapsed() > timeout {
            panic!("Timed out waiting for cloudflared tunnel URL");
        }
    }

    panic!("cloudflared exited without providing a tunnel URL");
}
