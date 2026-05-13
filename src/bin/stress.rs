//! Stress test for things-api.
//!
//! Spawn the server first (e.g. `cargo run --release`), then run:
//!
//!   cargo run --release --bin stress -- --duration 15 --concurrency 8
//!
//! Created tasks are tagged `stress-test` and deleted at the end. Anything
//! that escapes cleanup can be found by searching that tag in Things 3.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::{Parser, ValueEnum};
use rand::Rng;
use reqwest::{Client, Method, StatusCode};
use serde_json::json;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;

#[derive(Parser)]
#[command(name = "stress", about = "Stress test for things-api")]
struct Args {
    /// Base URL of a running things-api server
    #[arg(long, default_value = "http://127.0.0.1:3333")]
    base_url: String,

    /// Bearer token (only required if the server requires auth, e.g. --tunnel)
    #[arg(long, env = "THINGS_AUTH_TOKEN")]
    token: Option<String>,

    /// Scenario to run
    #[arg(long, value_enum, default_value_t = Scenario::All)]
    scenario: Scenario,

    /// Duration in seconds for each load scenario
    #[arg(long, default_value_t = 15)]
    duration: u64,

    /// Concurrent in-flight requests for load scenarios
    #[arg(long, default_value_t = 8)]
    concurrency: usize,

    /// Number of samples per endpoint for the latency baseline
    #[arg(long, default_value_t = 5)]
    baseline_samples: usize,

    /// Skip cleanup of tasks created by the write/mixed scenarios
    #[arg(long)]
    no_cleanup: bool,

    /// After cleanup, also empty Things 3 trash so deleted tasks are purged
    #[arg(long)]
    empty_trash: bool,
}

#[derive(Copy, Clone, ValueEnum, Eq, PartialEq, Debug)]
enum Scenario {
    Baseline,
    Read,
    Write,
    Mixed,
    All,
}

struct WorkerCtx {
    client: Client,
    base: String,
    token: Option<String>,
    run_id: String,
}

impl WorkerCtx {
    fn request(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        let mut b = self
            .client
            .request(method, format!("{}{}", self.base, path));
        if let Some(t) = &self.token {
            b = b.bearer_auth(t);
        }
        b
    }
}

#[derive(Default)]
struct Stats {
    latencies: Vec<Duration>,
    successes: usize,
    failures: usize,
    status_codes: HashMap<u16, usize>,
}

impl Stats {
    fn new() -> Self {
        Self::default()
    }

    fn record(&mut self, status: StatusCode, elapsed: Duration) {
        self.latencies.push(elapsed);
        *self.status_codes.entry(status.as_u16()).or_insert(0) += 1;
        if status.is_success() {
            self.successes += 1;
        } else {
            self.failures += 1;
        }
    }

    fn record_error(&mut self, elapsed: Duration) {
        self.latencies.push(elapsed);
        self.failures += 1;
    }

    fn merge(&mut self, other: Stats) {
        self.latencies.extend(other.latencies);
        self.successes += other.successes;
        self.failures += other.failures;
        for (k, v) in other.status_codes {
            *self.status_codes.entry(k).or_insert(0) += v;
        }
    }

    fn report(&mut self, label: &str, total_duration: Duration) {
        let total = self.successes + self.failures;
        let secs = total_duration.as_secs_f64();
        let rps = if secs > 0.0 { total as f64 / secs } else { 0.0 };
        let success_pct = if total > 0 {
            100.0 * self.successes as f64 / total as f64
        } else {
            0.0
        };
        println!();
        println!("── {} ──", label);
        println!(
            "  requests:   {}  ({:.2} req/s over {:.1}s)",
            total, rps, secs
        );
        println!("  success:    {} ({:.1}%)", self.successes, success_pct);
        println!("  failure:    {}", self.failures);
        if !self.status_codes.is_empty() {
            let mut codes: Vec<_> = self.status_codes.iter().collect();
            codes.sort_by_key(|(c, _)| *c);
            let codes_str = codes
                .iter()
                .map(|(c, n)| format!("{}={}", c, n))
                .collect::<Vec<_>>()
                .join(" ");
            println!("  statuses:   {}", codes_str);
        }
        if !self.latencies.is_empty() {
            self.latencies.sort();
            let min = *self.latencies.first().unwrap();
            let max = *self.latencies.last().unwrap();
            let p50 = pct(&self.latencies, 50.0);
            let p90 = pct(&self.latencies, 90.0);
            let p99 = pct(&self.latencies, 99.0);
            let sum: Duration = self.latencies.iter().sum();
            let mean = sum / (self.latencies.len() as u32);
            println!(
                "  latency:    min {:>7}  mean {:>7}  p50 {:>7}  p90 {:>7}  p99 {:>7}  max {:>7}",
                fmt(min),
                fmt(mean),
                fmt(p50),
                fmt(p90),
                fmt(p99),
                fmt(max)
            );
        }
    }
}

fn pct(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let idx = ((sorted.len() as f64) * p / 100.0) as usize;
    let idx = idx.min(sorted.len() - 1);
    sorted[idx]
}

fn fmt(d: Duration) -> String {
    let ms = d.as_secs_f64() * 1000.0;
    if ms < 1.0 {
        format!("{}µs", d.as_micros())
    } else if ms < 1000.0 {
        format!("{:.1}ms", ms)
    } else {
        format!("{:.2}s", d.as_secs_f64())
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Args::parse();

    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .expect("build reqwest client");

    let run_id = format!("{:08x}", rand::rng().random::<u32>());
    let ctx = Arc::new(WorkerCtx {
        client,
        base: args.base_url.trim_end_matches('/').to_string(),
        token: args.token.clone(),
        run_id: run_id.clone(),
    });

    println!("things-api stress test");
    println!("  base:        {}", ctx.base);
    println!("  scenario:    {:?}", args.scenario);
    println!("  concurrency: {}", args.concurrency);
    println!("  duration:    {}s", args.duration);
    println!("  run_id:      {}", run_id);
    if args.token.is_some() {
        println!("  auth:        bearer token");
    }

    match ctx.request(Method::GET, "/health").send().await {
        Ok(r) if r.status().is_success() => {}
        Ok(r) => {
            eprintln!("Preflight failed: GET /health → {}", r.status());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Preflight failed: cannot reach {} ({})", ctx.base, e);
            std::process::exit(1);
        }
    }

    let dur = Duration::from_secs(args.duration);
    let created: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let run = args.scenario;

    if matches!(run, Scenario::Baseline | Scenario::All) {
        baseline(Arc::clone(&ctx), args.baseline_samples).await;
    }
    if matches!(run, Scenario::Read | Scenario::All) {
        run_read(Arc::clone(&ctx), args.concurrency, dur).await;
    }
    if matches!(run, Scenario::Write | Scenario::All) {
        run_write(Arc::clone(&ctx), args.concurrency, dur, Arc::clone(&created)).await;
    }
    if matches!(run, Scenario::Mixed | Scenario::All) {
        run_mixed(Arc::clone(&ctx), args.concurrency, dur, Arc::clone(&created)).await;
    }

    let ids = std::mem::take(&mut *created.lock().await);
    if args.no_cleanup {
        if !ids.is_empty() {
            println!();
            println!(
                "⚠ Cleanup skipped — {} created task(s) remain (search tag `stress-test`)",
                ids.len()
            );
        }
    } else if !ids.is_empty() {
        cleanup(Arc::clone(&ctx), ids).await;
    }

    if args.empty_trash {
        println!();
        println!("Emptying Things 3 trash...");
        let t = Instant::now();
        match ctx.request(Method::DELETE, "/trash").send().await {
            Ok(r) if r.status().is_success() => {
                println!("  trash emptied in {:.1}s", t.elapsed().as_secs_f64());
            }
            Ok(r) => eprintln!("  empty-trash failed: HTTP {}", r.status()),
            Err(e) => eprintln!("  empty-trash failed: {}", e),
        }
    }
}

async fn baseline(ctx: Arc<WorkerCtx>, samples: usize) {
    let endpoints: &[(&str, Method, &str)] = &[
        ("GET /tasks", Method::GET, "/tasks"),
        ("GET /projects", Method::GET, "/projects"),
        ("GET /tags", Method::GET, "/tags"),
        ("GET /areas", Method::GET, "/areas"),
    ];

    println!();
    println!(
        "=== Latency baseline (single-flight, {} samples per endpoint) ===",
        samples
    );

    for (label, method, path) in endpoints {
        let mut stats = Stats::new();
        let start = Instant::now();
        for _ in 0..samples {
            let t = Instant::now();
            match ctx.request(method.clone(), path).send().await {
                Ok(r) => {
                    let status = r.status();
                    let _ = r.bytes().await;
                    stats.record(status, t.elapsed());
                }
                Err(_) => stats.record_error(t.elapsed()),
            }
        }
        stats.report(label, start.elapsed());
    }
}

async fn run_read(ctx: Arc<WorkerCtx>, concurrency: usize, duration: Duration) {
    println!();
    println!(
        "=== Read-heavy scenario (concurrency {}, {}s) ===",
        concurrency,
        duration.as_secs()
    );

    let paths = ["/tasks", "/projects", "/tags", "/areas"];
    let deadline = Instant::now() + duration;
    let mut set: JoinSet<Stats> = JoinSet::new();

    for w in 0..concurrency {
        let ctx = Arc::clone(&ctx);
        set.spawn(async move {
            let mut local = Stats::new();
            let mut i = w;
            while Instant::now() < deadline {
                let path = paths[i % paths.len()];
                i += 1;
                let t = Instant::now();
                match ctx.request(Method::GET, path).send().await {
                    Ok(r) => {
                        let status = r.status();
                        let _ = r.bytes().await;
                        local.record(status, t.elapsed());
                    }
                    Err(_) => local.record_error(t.elapsed()),
                }
            }
            local
        });
    }

    let start = Instant::now();
    let mut total = Stats::new();
    while let Some(joined) = set.join_next().await {
        if let Ok(s) = joined {
            total.merge(s);
        }
    }
    total.report("read-heavy", start.elapsed());
}

async fn run_write(
    ctx: Arc<WorkerCtx>,
    concurrency: usize,
    duration: Duration,
    created: Arc<Mutex<Vec<String>>>,
) {
    println!();
    println!(
        "=== Write-heavy scenario (POST /tasks, concurrency {}, {}s) ===",
        concurrency,
        duration.as_secs()
    );

    let deadline = Instant::now() + duration;
    let mut set: JoinSet<Stats> = JoinSet::new();

    for w in 0..concurrency {
        let ctx = Arc::clone(&ctx);
        let created = Arc::clone(&created);
        set.spawn(async move {
            let mut local = Stats::new();
            let mut n = 0u64;
            while Instant::now() < deadline {
                let body = json!({
                    "title": format!("[stress {} w{}] task {}", ctx.run_id, w, n),
                    "tags": ["stress-test"],
                });
                n += 1;
                let t = Instant::now();
                match ctx.request(Method::POST, "/tasks").json(&body).send().await {
                    Ok(r) => {
                        let status = r.status();
                        let body: serde_json::Value =
                            r.json().await.unwrap_or(serde_json::Value::Null);
                        if status.is_success() {
                            if let Some(id) = body.get("id").and_then(|v| v.as_str()) {
                                created.lock().await.push(id.to_string());
                            }
                        }
                        local.record(status, t.elapsed());
                    }
                    Err(_) => local.record_error(t.elapsed()),
                }
            }
            local
        });
    }

    let start = Instant::now();
    let mut total = Stats::new();
    while let Some(joined) = set.join_next().await {
        if let Ok(s) = joined {
            total.merge(s);
        }
    }
    total.report("write-heavy", start.elapsed());
}

async fn run_mixed(
    ctx: Arc<WorkerCtx>,
    concurrency: usize,
    duration: Duration,
    created: Arc<Mutex<Vec<String>>>,
) {
    println!();
    println!(
        "=== Mixed scenario (80% read / 20% write, concurrency {}, {}s) ===",
        concurrency,
        duration.as_secs()
    );

    let read_paths = ["/tasks", "/projects", "/tags", "/areas"];
    let deadline = Instant::now() + duration;
    let mut set: JoinSet<Stats> = JoinSet::new();

    for w in 0..concurrency {
        let ctx = Arc::clone(&ctx);
        let created = Arc::clone(&created);
        set.spawn(async move {
            let mut local = Stats::new();
            let mut n = 0u64;
            while Instant::now() < deadline {
                let is_write = rand::rng().random_ratio(1, 5);
                let t = Instant::now();
                if is_write {
                    let body = json!({
                        "title": format!("[stress {} m{}] task {}", ctx.run_id, w, n),
                        "tags": ["stress-test"],
                    });
                    n += 1;
                    match ctx.request(Method::POST, "/tasks").json(&body).send().await {
                        Ok(r) => {
                            let status = r.status();
                            let body: serde_json::Value =
                                r.json().await.unwrap_or(serde_json::Value::Null);
                            if status.is_success() {
                                if let Some(id) = body.get("id").and_then(|v| v.as_str()) {
                                    created.lock().await.push(id.to_string());
                                }
                            }
                            local.record(status, t.elapsed());
                        }
                        Err(_) => local.record_error(t.elapsed()),
                    }
                } else {
                    let path = read_paths[(n as usize) % read_paths.len()];
                    n += 1;
                    match ctx.request(Method::GET, path).send().await {
                        Ok(r) => {
                            let status = r.status();
                            let _ = r.bytes().await;
                            local.record(status, t.elapsed());
                        }
                        Err(_) => local.record_error(t.elapsed()),
                    }
                }
            }
            local
        });
    }

    let start = Instant::now();
    let mut total = Stats::new();
    while let Some(joined) = set.join_next().await {
        if let Ok(s) = joined {
            total.merge(s);
        }
    }
    total.report("mixed 80/20", start.elapsed());
}

async fn cleanup(ctx: Arc<WorkerCtx>, ids: Vec<String>) {
    let total = ids.len();
    println!();
    println!("=== Cleanup ===");
    println!("  deleting {} created task(s)...", total);
    let start = Instant::now();

    let permits = Arc::new(Semaphore::new(4));
    let mut set: JoinSet<bool> = JoinSet::new();
    for id in ids {
        let ctx = Arc::clone(&ctx);
        let permit = Arc::clone(&permits)
            .acquire_owned()
            .await
            .expect("semaphore closed");
        set.spawn(async move {
            let _p = permit;
            ctx.request(Method::DELETE, &format!("/tasks/{}", id))
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        });
    }

    let mut ok = 0usize;
    while let Some(joined) = set.join_next().await {
        if let Ok(true) = joined {
            ok += 1;
        }
    }
    println!(
        "  deleted {} / {} in {:.1}s",
        ok,
        total,
        start.elapsed().as_secs_f64()
    );
    if ok < total {
        println!(
            "  ⚠ {} task(s) failed to delete — search Things 3 for tag `stress-test`",
            total - ok
        );
    }
}
