//! Refresh all listings via the AgentZero API.
//!
//! For each listing, fetches a live preview of the refreshed data, shows what
//! would change, then asks for confirmation (unless --auto-accept is set).
//!
//! Usage:
//!   cargo run --bin refresh_all
//!   cargo run --bin refresh_all -- --status Interested,Buyable
//!   cargo run --bin refresh_all -- --auto-accept
//!   cargo run --bin refresh_all -- --dry-run
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

    /// Refresh all listings without per-listing confirmation prompts
    #[arg(long)]
    auto_accept: bool,

    /// Show what would change without actually refreshing
    #[arg(long)]
    dry_run: bool,
}

#[derive(Deserialize, Clone)]
struct Listing {
    id: i64,
    title: String,
    status: String,
    price: Option<i64>,
    price_currency: Option<String>,
    street_address: Option<String>,
    bedrooms: Option<i64>,
    bathrooms: Option<i64>,
    property_tax: Option<i64>,
    hoa_monthly: Option<i64>,
    listed_date: Option<String>,
    mls_number: Option<String>,
}

struct Change {
    label: &'static str,
    from: String,
    to: String,
}

fn fmt_dollars(cents: &i64) -> String {
    let n = *cents as u64;
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3 + 1);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    format!("${}", out.chars().rev().collect::<String>())
}

fn fmt_opt_str(v: &Option<String>) -> String {
    v.as_deref().unwrap_or("—").to_string()
}

fn fmt_opt_i64(v: &Option<i64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_else(|| "—".to_string())
}

fn compute_diff(current: &Listing, preview: &Listing) -> Vec<Change> {
    let mut changes = Vec::new();

    macro_rules! field {
        ($label:expr, $field:ident, $fmt:expr) => {
            if current.$field != preview.$field {
                changes.push(Change {
                    label: $label,
                    from: $fmt(&current.$field),
                    to: $fmt(&preview.$field),
                });
            }
        };
    }

    // title is non-optional
    if current.title != preview.title {
        changes.push(Change {
            label: "title",
            from: current.title.clone(),
            to: preview.title.clone(),
        });
    }

    field!("street_address", street_address, fmt_opt_str);
    field!("price_currency", price_currency, fmt_opt_str);

    if current.price != preview.price {
        changes.push(Change {
            label: "price",
            from: current.price.as_ref().map(fmt_dollars).unwrap_or_else(|| "—".to_string()),
            to: preview.price.as_ref().map(fmt_dollars).unwrap_or_else(|| "—".to_string()),
        });
    }

    field!("bedrooms", bedrooms, fmt_opt_i64);
    field!("bathrooms", bathrooms, fmt_opt_i64);

    if current.property_tax != preview.property_tax {
        changes.push(Change {
            label: "property_tax",
            from: current.property_tax.as_ref().map(fmt_dollars).unwrap_or_else(|| "—".to_string()),
            to: preview.property_tax.as_ref().map(fmt_dollars).unwrap_or_else(|| "—".to_string()),
        });
    }

    if current.hoa_monthly != preview.hoa_monthly {
        changes.push(Change {
            label: "hoa_monthly",
            from: current.hoa_monthly.as_ref().map(fmt_dollars).unwrap_or_else(|| "—".to_string()),
            to: preview.hoa_monthly.as_ref().map(fmt_dollars).unwrap_or_else(|| "—".to_string()),
        });
    }

    field!("listed_date", listed_date, fmt_opt_str);
    field!("mls_number", mls_number, fmt_opt_str);

    changes
}

fn print_diff(changes: &[Change]) {
    if changes.is_empty() {
        println!("  no changes detected");
        return;
    }
    let label_width = changes.iter().map(|c| c.label.len()).max().unwrap_or(0);
    for c in changes {
        println!("  {:<width$}  {}  →  {}", c.label, c.from, c.to, width = label_width);
    }
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

        // Fetch preview to compute diff
        print!("  previewing...");
        io::stdout().flush()?;

        let preview_url = format!("{base_url}/api/listings/{}/preview", listing.id);
        let preview_result = client.get(&preview_url).send().await;

        match preview_result {
            Err(e) => {
                println!(" FAILED: {e}");
                if !cli.auto_accept && !cli.dry_run {
                    print!("  Refresh anyway? [y/N] ");
                    io::stdout().flush()?;
                    let mut line = String::new();
                    stdin_lock.read_line(&mut line)?;
                    if !matches!(line.trim(), "y" | "Y") {
                        println!("  skipped");
                        skipped += 1;
                        continue;
                    }
                } else if cli.dry_run {
                    continue;
                }
            }
            Ok(resp) if !resp.status().is_success() => {
                println!(" {} — skipping", resp.status());
                skipped += 1;
                continue;
            }
            Ok(resp) => {
                println!();
                match resp.json::<Listing>().await {
                    Ok(preview) => {
                        let changes = compute_diff(listing, &preview);
                        print_diff(&changes);
                    }
                    Err(e) => {
                        println!("  (could not parse preview: {e})");
                    }
                }

                if cli.dry_run {
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
        println!("[refresh-all] dry-run complete — {total} listing(s) previewed");
    } else {
        println!("[refresh-all] done — refreshed={refreshed} skipped={skipped} failed={failed}");
    }

    Ok(())
}
