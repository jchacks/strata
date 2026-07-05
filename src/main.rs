use crate::t212::{PieDetailed, PieSummary, Trading212Client, Trading212Error};
use anyhow::Context;
use futures::{StreamExt, TryStreamExt, stream};
use serde::{Deserialize, Serialize};
use std::path::Path;

mod t212;

const FULL_PIE_INFO_CACHE_PATH: &str = "cache/t212/full_pie_info.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FullPieInfo {
    summary: PieSummary,
    detail: PieDetailed,
}

async fn fetch_all_pie_details(
    client: &Trading212Client,
) -> Result<Vec<FullPieInfo>, Trading212Error> {
    let pies = client.get_pies().await?;

    stream::iter(pies)
        .map(|summary| async move {
            let detail = client.get_pie(summary.id).await?;
            Ok(FullPieInfo { summary, detail })
        })
        .buffer_unordered(1)
        .try_collect::<Vec<_>>()
        .await
}

async fn get_all_pie_details(client: &Trading212Client) -> anyhow::Result<Vec<FullPieInfo>> {
    let cache_path = Path::new(FULL_PIE_INFO_CACHE_PATH);

    if cache_path.exists() {
        let cached = std::fs::read_to_string(cache_path)
            .with_context(|| format!("failed to read cache file {FULL_PIE_INFO_CACHE_PATH}"))?;

        let full_info = serde_json::from_str(&cached)
            .with_context(|| format!("failed to parse cache file {FULL_PIE_INFO_CACHE_PATH}"))?;

        return Ok(full_info);
    }

    let full_info = fetch_all_pie_details(client)
        .await
        .context("failed to fetch Trading 212 pie details")?;

    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create cache directory {}", parent.display()))?;
    }

    let json = serde_json::to_string_pretty(&full_info)
        .context("failed to serialize full pie info cache")?;

    std::fs::write(cache_path, json)
        .with_context(|| format!("failed to write cache file {FULL_PIE_INFO_CACHE_PATH}"))?;

    Ok(full_info)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let api_key = std::env::var("TRADING212_API_KEY").context("TRADING212_API_KEY is not set")?;
    let api_secret =
        std::env::var("TRADING212_API_SECRET").context("TRADING212_API_SECRET is not set")?;

    let t212_client =
        Trading212Client::new("https://live.trading212.com/api/v0/", api_key, api_secret)
            .context("failed to create Trading 212 client")?;

    let pies = get_all_pie_details(&t212_client).await?;
    println!("{pies:#?}");

    Ok(())
}
