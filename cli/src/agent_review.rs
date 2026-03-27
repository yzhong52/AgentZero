//! Trigger and inspect agent review for a listing.
//!
//! Usage:
//!   cargo run --bin agent_review -- --id 89
//!   BACKEND_PORT=8000 cargo run --bin agent_review -- --id 89
//!
//! Triggers the agent review for the given listing, then polls until the result
//! arrives (or times out), and prints the outcome.

use clap::Parser;
use serde::Deserialize;
use std::env;
use std::time::Instant;

const DEFAULT_PORT: u16 = 8000;
const POLL_INTERVAL_MS: u64 = 500;
const TIMEOUT_SECS: u64 = 30;

const BOLD:   &str = "\x1b[1m";
const DIM:    &str = "\x1b[2m";
const RESET:  &str = "\x1b[0m";
const GREEN:  &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED:    &str = "\x1b[31m";
const CYAN:   &str = "\x1b[36m";

#[derive(Parser)]
#[command(name = "agent_review")]
struct Cli {
    /// Listing ID to run agent review on.
    #[arg(long, value_name = "ID")]
    id: i64,
}

#[derive(Deserialize, Clone)]
struct Listing {
    id: i64,
    title: String,
    status: String,
    agent_comment: Option<String>,
    search_profile_id: Option<i64>,
}

fn fmt_status(s: &str) -> String {
    match s {
        "Buyable"      => format!("{GREEN}{BOLD}{s}{RESET}"),
        "Interested"   => format!("{CYAN}{s}{RESET}"),
        "HumanReview"  => format!("{YELLOW}HumanReview{RESET}"),
        "AgentPending" => format!("{DIM}AgentPending{RESET}"),
        "AgentSkip"    => format!("{DIM}AgentSkip{RESET}"),
        "Pass"         => format!("{DIM}{s}{RESET}"),
        _              => s.to_string(),
    }
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let port: u16 = env::var("BACKEND_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let base_url = format!("http://127.0.0.1:{port}");

    let client = reqwest::Client::new();

    // ── Fetch current state ────────────────────────────────────────────────────
    let listing_url = format!("{base_url}/api/listings/{}", cli.id);
    let before: Listing = client
        .get(&listing_url)
        .send()
        .await
        .map_err(|e| format!("Failed to reach backend at {base_url}: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Listing #{} not found: {e}", cli.id))?
        .json()
        .await?;

    println!(
        "[agent-review] #{} — {BOLD}{}{RESET}",
        before.id, before.title
    );
    println!(
        "  before: status={} comment={}",
        fmt_status(&before.status),
        before.agent_comment.as_deref()
            .map(|c| format!("{DIM}\"{c}\"{RESET}"))
            .unwrap_or_else(|| format!("{DIM}—{RESET}")),
    );

    // ── Trigger review ─────────────────────────────────────────────────────────
    let trigger_url = format!("{base_url}/api/listings/{}/agent-review/run", cli.id);
    println!("[agent-review] triggering…");
    let trigger_resp = client
        .post(&trigger_url)
        .send()
        .await
        .map_err(|e| format!("Trigger request failed: {e}"))?;

    if !trigger_resp.status().is_success() {
        let status = trigger_resp.status();
        let body = trigger_resp.text().await.unwrap_or_default();
        eprintln!("{RED}[agent-review] trigger failed (HTTP {status}): {body}{RESET}");
        std::process::exit(1);
    }

    // ── Poll for result ────────────────────────────────────────────────────────
    print!("[agent-review] waiting");
    let start = Instant::now();

    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
        print!(".");

        let resp = client.get(&listing_url).send().await;
        let after: Listing = match resp {
            Ok(r) if r.status().is_success() => match r.json().await {
                Ok(l) => l,
                Err(_) => continue,
            },
            _ => continue,
        };

        let changed = after.agent_comment != before.agent_comment
            || after.status != before.status;

        if changed {
            let elapsed = start.elapsed().as_secs_f32();
            println!(" done ({elapsed:.1}s)");
            println!(
                "  after:  status={} comment={}",
                fmt_status(&after.status),
                after.agent_comment.as_deref()
                    .map(|c| format!("{BOLD}\"{c}\"{RESET}"))
                    .unwrap_or_else(|| format!("{DIM}—{RESET}")),
            );
            if let Some(pid) = after.search_profile_id {
                println!("          search_profile_id={pid}");
            }
            return Ok(());
        }

        if start.elapsed().as_secs() >= TIMEOUT_SECS {
            println!();
            eprintln!(
                "{RED}[agent-review] timed out after {TIMEOUT_SECS}s — no change detected{RESET}"
            );
            eprintln!(
                "{DIM}Check the backend log for errors:{RESET}"
            );
            eprintln!(
                "{DIM}  grep agent_review /tmp/agent_zero_backend.log | tail -20{RESET}"
            );
            std::process::exit(1);
        }
    }
}
