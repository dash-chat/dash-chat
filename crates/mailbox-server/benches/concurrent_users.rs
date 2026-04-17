//! Benchmark: how many concurrent users can the mailbox server support?
//!
//! Starts a real HTTP server, simulates N concurrent users each performing
//! periodic store + get operations, and reports throughput and latency
//! percentiles (p50, p95, p99) at each concurrency level.
//!
//! Run with:
//!   cargo bench -p mailbox-server --bench concurrent_users
//!
//! Or for a quick smoke test (fewer concurrency levels):
//!   cargo bench -p mailbox-server --bench concurrent_users -- --quick

use mailbox_server::init_db;
use reqwest::Client;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::Barrier;

/// A single recorded request latency.
#[derive(Debug, Clone, Copy)]
struct Sample {
    latency: Duration,
    success: bool,
}

/// Results from one concurrency level.
struct BenchResult {
    concurrent_users: usize,
    total_requests: u64,
    successful: u64,
    failed: u64,
    #[allow(dead_code)]
    elapsed: Duration,
    p50: Duration,
    p95: Duration,
    p99: Duration,
    max: Duration,
    requests_per_sec: f64,
}

impl std::fmt::Display for BenchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "  users: {:>5} | reqs: {:>6} | ok: {:>6} | fail: {:>4} | rps: {:>8.0} | \
             p50: {:>6.1}ms | p95: {:>6.1}ms | p99: {:>6.1}ms | max: {:>8.1}ms",
            self.concurrent_users,
            self.total_requests,
            self.successful,
            self.failed,
            self.requests_per_sec,
            self.p50.as_secs_f64() * 1000.0,
            self.p95.as_secs_f64() * 1000.0,
            self.p99.as_secs_f64() * 1000.0,
            self.max.as_secs_f64() * 1000.0,
        )
    }
}

/// Start a real mailbox server on a random port, return the base URL.
async fn start_server() -> (String, tempfile::NamedTempFile) {
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let db = init_db(temp_file.path().to_path_buf()).unwrap();
    let app = mailbox_server::create_app(db);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Wait for the server to be ready.
    let url = format!("http://{}", addr);
    let client = Client::new();
    for _ in 0..50 {
        if client
            .get(format!("{}/health", url))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    (url, temp_file)
}

/// Simulate one user session: alternate between storing a message and
/// retrieving messages, collecting latency samples.
async fn user_session(
    client: &Client,
    base_url: &str,
    user_id: usize,
    num_requests: usize,
    barrier: Arc<Barrier>,
    seq_counter: Arc<AtomicU64>,
) -> Vec<Sample> {
    let topic_id = format!("bench-topic-{}", user_id % 50);
    let author = format!("user-{}", user_id);
    let mut samples = Vec::with_capacity(num_requests);

    // All users start at the same time.
    barrier.wait().await;

    for i in 0..num_requests {
        if i % 2 == 0 {
            // Store a message.
            let seq = seq_counter.fetch_add(1, Ordering::Relaxed);
            let payload = serde_json::json!({
                "blobs": {
                    &topic_id: {
                        &author: {
                            seq.to_string(): base64::Engine::encode(
                                &base64::engine::general_purpose::STANDARD,
                                format!("msg-{}-{}", user_id, i).as_bytes(),
                            )
                        }
                    }
                }
            });

            let start = Instant::now();
            let resp = client
                .post(format!("{}/blobs/store", base_url))
                .json(&payload)
                .send()
                .await;
            let latency = start.elapsed();

            samples.push(Sample {
                latency,
                success: resp.map(|r| r.status().is_success()).unwrap_or(false),
            });
        } else {
            // Get messages.
            let payload = serde_json::json!({
                "topics": {
                    &topic_id: {}
                }
            });

            let start = Instant::now();
            let resp = client
                .post(format!("{}/blobs/get", base_url))
                .json(&payload)
                .send()
                .await;
            let latency = start.elapsed();

            samples.push(Sample {
                latency,
                success: resp.map(|r| r.status().is_success()).unwrap_or(false),
            });
        }
    }

    samples
}

fn percentile(sorted: &[Duration], pct: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let idx = ((sorted.len() as f64) * pct / 100.0).ceil() as usize;
    sorted[idx.saturating_sub(1).min(sorted.len() - 1)]
}

async fn run_benchmark(
    base_url: &str,
    concurrent_users: usize,
    requests_per_user: usize,
) -> BenchResult {
    let client = Client::builder()
        .pool_max_idle_per_host(concurrent_users)
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();

    let barrier = Arc::new(Barrier::new(concurrent_users));
    let seq_counter = Arc::new(AtomicU64::new(0));

    let start = Instant::now();

    let mut handles = Vec::with_capacity(concurrent_users);
    for user_id in 0..concurrent_users {
        let client = client.clone();
        let url = base_url.to_string();
        let barrier = Arc::clone(&barrier);
        let seq_counter = Arc::clone(&seq_counter);

        handles.push(tokio::spawn(async move {
            user_session(&client, &url, user_id, requests_per_user, barrier, seq_counter).await
        }));
    }

    let mut all_samples = Vec::with_capacity(concurrent_users * requests_per_user);
    for handle in handles {
        all_samples.extend(handle.await.unwrap());
    }

    let elapsed = start.elapsed();

    let successful = all_samples.iter().filter(|s| s.success).count() as u64;
    let failed = all_samples.iter().filter(|s| !s.success).count() as u64;

    let mut latencies: Vec<Duration> = all_samples.iter().map(|s| s.latency).collect();
    latencies.sort();

    BenchResult {
        concurrent_users,
        total_requests: all_samples.len() as u64,
        successful,
        failed,
        elapsed,
        p50: percentile(&latencies, 50.0),
        p95: percentile(&latencies, 95.0),
        p99: percentile(&latencies, 99.0),
        max: latencies.last().copied().unwrap_or(Duration::ZERO),
        requests_per_sec: all_samples.len() as f64 / elapsed.as_secs_f64(),
    }
}

fn main() {
    let quick = std::env::args().any(|a| a == "--quick");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        let (_base_url, _temp_file) = start_server().await;

        let levels: Vec<usize> = if quick {
            vec![1, 10, 50, 100]
        } else {
            vec![1, 10, 50, 100, 200, 500, 1000]
        };

        let requests_per_user = if quick { 10 } else { 20 };

        println!();
        println!("=== Mailbox Server Benchmark: Concurrent Users ===");
        println!("  requests per user: {}", requests_per_user);
        println!();

        let mut results = Vec::new();

        for &users in &levels {
            // Fresh server for each level to avoid accumulated data skewing results.
            let (url, _tf) = start_server().await;
            let result = run_benchmark(&url, users, requests_per_user).await;
            println!("{}", result);
            results.push(result);
        }

        // Summary: find the highest concurrency where p99 < 500ms and error rate < 1%.
        println!();
        println!("=== Summary ===");

        let max_sustainable = results
            .iter()
            .filter(|r| {
                r.failed == 0
                    && r.p99 < Duration::from_millis(500)
            })
            .last();

        if let Some(best) = max_sustainable {
            println!(
                "  Max tested concurrency with p99 < 500ms and 0 failures: {} users",
                best.concurrent_users
            );
            println!(
                "  At that level: {:.0} req/s, p99 = {:.1}ms",
                best.requests_per_sec,
                best.p99.as_secs_f64() * 1000.0,
            );
        } else {
            println!("  Server struggled at all concurrency levels.");
        }

        // Also report the first level where things degrade.
        let first_degraded = results
            .iter()
            .find(|r| r.failed > 0 || r.p99 >= Duration::from_millis(500));

        if let Some(degraded) = first_degraded {
            println!(
                "  First degradation at {} users: p99 = {:.1}ms, {} failures",
                degraded.concurrent_users,
                degraded.p99.as_secs_f64() * 1000.0,
                degraded.failed,
            );
        } else {
            println!(
                "  No degradation observed up to {} users — consider testing higher levels.",
                results.last().map(|r| r.concurrent_users).unwrap_or(0)
            );
        }

        println!();
    });
}
