//! Refresh all listings via the AgentZero API.
//!
//! Usage:
//!   cargo run --bin refresh_all
//!   cargo run --bin refresh_all -- --status Interested,Buyable
//!   cargo run --bin refresh_all -- --auto-accept
//!   cargo run --bin refresh_all -- --dry-run
//!   cargo run --bin refresh_all -- --status Interested --auto-accept
//!
//! Reads BACKEND_PORT from the environment (default: 8000).

use clap::Parser;
use serde::Deserialize;
use std::env;
use std::io::{self, BufRead, Write};

const DEFAULT_PORT: u16 = 8000;

/// Refresh all listings from their source URLs.
#[derive(Parser)]
#[command(name = "refresh_all")]
struct Cli {
    /// Only refresh listings with these statuses (comma-separated).
    /// Valid values: Interested, Buyable, Pending, Pass
    #[arg(long, value_name = "STATUS,...")]
    status: Option<String>,

    /// Skip per-listing confirmation prompts
    #[arg(long)]
    auto_accept: bool,

    /// List what would be refreshed without making any changes
    #[arg(long)]
    dry_run: bool,
}

#[derive(Deserialize)]
struct Listing {
    id: i64,
    title: String,
    status: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let port: u16 = env::var("BACKEND_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let base_url = format!("http://127.0.0.1:{port}");

    let listings_url = match &cli.status {
        Some(s) => format!("{base_url}/api/listings?status={s}"),
        None => format!("{base_url}/api/listings"),
    };

    println!("[refresh-all] fetching listings from {listings_url}...");

    let client = reqwest::Client::new();
    let response = client
        .get(&listings_url)
        .send()
        .await
        .map_err(|e| format!("Failed to reach backend at {base_url}: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("Backend returned {}", response.status()).into());
    }

    let listings: Vec<Listing> = response.json().await?;
    let total = listings.len();

    if total == 0 {
        println!("[refresh-all] no listings found");
        return Ok(());
    }

    println!("[refresh-all] found {total} listing(s)");
    if cli.dry_run {
        println!("[refresh-all] dry-run mode — no changes will be made");
    } else if cli.auto_accept {
        println!("[refresh-all] auto-accept mode — refreshing all without prompts");
    }

    let mut refreshed = 0u32;
    let mut skipped = 0u32;
    let mut failed = 0u32;

    let stdin = io::stdin();
    let mut stdin_lock = stdin.lock();

    for (index, listing) in listings.iter().enumerate() {
        println!(
            "\n  [{}/{}] #{} — {} (status: {})",
            index + 1,
            total,
            listing.id,
            listing.title,
            listing.status,
        );

        if cli.dry_run {
            println!("  [dry-run] would refresh");
            continue;
        }

        if !cli.auto_accept {
            print!("  Refresh? [y/N] ");
            io::stdout().flush()?;
            let mut line = String::new();
            stdin_lock.read_line(&mut line)?;
            if !matches!(line.trim(), "y" | "Y") {
                println!("  skipped");
                skipped += 1;
                continue;
            }
        }

        let refresh_url = format!("{base_url}/api/listings/{}/refresh", listing.id);
        match client.put(&refresh_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                println!("  refreshed (HTTP {})", resp.status().as_u16());
                refreshed += 1;
            }
            Ok(resp) => {
                eprintln!("  FAILED (HTTP {})", resp.status().as_u16());
                failed += 1;
            }
            Err(e) => {
                eprintln!("  FAILED: {e}");
                failed += 1;
            }
        }
    }

    println!();
    if cli.dry_run {
        println!("[refresh-all] dry-run complete — {total} listing(s) would be refreshed");
    } else {
        println!("[refresh-all] done — refreshed={refreshed} skipped={skipped} failed={failed}");
    }

    Ok(())
}
