use std::{path::PathBuf, sync::Arc};

use anyhow::Context;
use async_trait::async_trait;
use futures::{StreamExt, TryStreamExt, stream};
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    tools::{ToolRegistry, TypedTool},
    trading212::{PieDetailed, PieSummary, Trading212Client, Trading212Error},
};

pub struct Trading212Toolset {
    client: Arc<Trading212Client>,
    cache_dir: PathBuf,
}

impl Trading212Toolset {
    pub fn new(client: Trading212Client, cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            client: Arc::new(client),
            cache_dir: cache_dir.into(),
        }
    }

    pub fn register_tools(&self, registry: &mut ToolRegistry) {
        tracing::debug!("registering Trading 212 toolset");
        registry.register(GetPortfolioSummaryTool::new(
            self.client.clone(),
            self.cache_dir.join("t212/full_pie_info.json"),
        ));
    }
}

/// Trading 212 pie summary paired with its detailed instrument information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FullPieInfo {
    /// Summary fields returned by the fetch-all-pies endpoint.
    pub summary: PieSummary,
    /// Detailed fields returned by the fetch-pie endpoint.
    pub detail: PieDetailed,
}

async fn get_all_pie_details_cached(
    client: &Trading212Client,
    cache_path: &PathBuf,
) -> anyhow::Result<Vec<FullPieInfo>> {
    if cache_path.exists() {
        tracing::info!(cache_path = %cache_path.display(), "Trading 212 pie cache hit");
        let cached = std::fs::read_to_string(cache_path)
            .with_context(|| format!("failed to read cache file {}", cache_path.display()))?;

        let full_info = serde_json::from_str(&cached)
            .with_context(|| format!("failed to parse cache file {}", cache_path.display()))?;

        return Ok(full_info);
    }

    tracing::info!(cache_path = %cache_path.display(), "Trading 212 pie cache miss");

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
        .with_context(|| format!("failed to write cache file {}", cache_path.display()))?;

    tracing::info!(cache_path = %cache_path.display(), "Trading 212 pie cache written");

    Ok(full_info)
}

async fn fetch_all_pie_details(
    client: &Trading212Client,
) -> Result<Vec<FullPieInfo>, Trading212Error> {
    tracing::info!("fetching Trading 212 pie summaries");
    let pies = client.get_pies().await?;
    tracing::info!(pie_count = pies.len(), "fetched Trading 212 pie summaries");

    stream::iter(pies)
        .map(|summary| async move {
            tracing::debug!(pie_id = summary.id, "fetching Trading 212 pie detail");
            let detail = client.get_pie(summary.id).await?;
            Ok(FullPieInfo { summary, detail })
        })
        .buffer_unordered(1)
        .try_collect::<Vec<_>>()
        .await
}

pub struct GetPortfolioSummaryTool {
    client: Arc<Trading212Client>,
    cache_path: PathBuf,
}

impl GetPortfolioSummaryTool {
    pub fn new(client: Arc<Trading212Client>, cache_path: PathBuf) -> Self {
        Self { client, cache_path }
    }
}

/// Input for the get_portfolio_summary tool.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetPortfolioSummaryInput {}

/// Portfolio summary derived from Trading 212 pie data.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GetPortfolioSummaryOutput {
    /// Source system used to fetch the underlying data.
    pub source: &'static str,
    /// True because this tool never modifies Trading 212 data.
    pub read_only: bool,
    /// Total current value used as the denominator for portfolio weights, rounded to whole account-currency units.
    pub total_current_value_rounded: Option<Decimal>,
    /// Pies with summarized instrument data.
    pub pies: Vec<PortfolioPieSummary>,
}

/// Pie-level portfolio summary.
#[derive(Debug, Serialize, JsonSchema)]
pub struct PortfolioPieSummary {
    /// Pie name as configured in Trading 212. May be user-defined.
    pub name: String,
    /// Current pie value rounded to whole account-currency units.
    pub current_value_rounded: Option<Decimal>,
    /// Pie weight as a percentage of the returned portfolio, rounded to two decimal places.
    pub portfolio_weight_percent: Option<Decimal>,
    /// Instruments held in the pie.
    pub instruments: Vec<PortfolioInstrumentSummary>,
}

/// Instrument-level portfolio summary.
#[derive(Debug, Serialize, JsonSchema)]
pub struct PortfolioInstrumentSummary {
    /// Trading 212 instrument ticker. This may need resolving to an exchange ticker/ISIN before look-through exposure calculations.
    pub ticker: String,
    /// Instrument current value rounded to whole account-currency units.
    pub current_value_rounded: Option<Decimal>,
    /// Instrument weight as a percentage of the returned portfolio, rounded to two decimal places.
    pub portfolio_weight_percent: Option<Decimal>,
    /// Instrument weight within its pie, rounded to two decimal places.
    pub pie_weight_percent: Option<Decimal>,
    /// Configured target share from Trading 212 pie settings, if returned by the API, rounded to two decimal places.
    pub target_share_percent: Option<Decimal>,
}

#[async_trait]
impl TypedTool for GetPortfolioSummaryTool {
    type Input = GetPortfolioSummaryInput;
    type Output = GetPortfolioSummaryOutput;

    fn name(&self) -> &'static str {
        "get_portfolio_summary"
    }

    fn description(&self) -> &'static str {
        "Returns a read-only Trading 212 portfolio summary with pie names, instrument tickers, rounded current values, and deterministic portfolio/pie weights. Does not compute ETF look-through exposure."
    }

    async fn call_typed(&self, _input: Self::Input) -> anyhow::Result<Self::Output> {
        tracing::info!("building portfolio summary from Trading 212 data");
        let full_info = get_all_pie_details_cached(&self.client, &self.cache_path).await?;
        Ok(build_portfolio_summary(full_info))
    }
}

fn build_portfolio_summary(full_info: Vec<FullPieInfo>) -> GetPortfolioSummaryOutput {
    let total_current_value = full_info
        .iter()
        .filter_map(|pie| pie.summary.result.price_avg_value)
        .sum::<Decimal>();

    let total_current_value = if total_current_value.is_zero() {
        None
    } else {
        Some(total_current_value)
    };

    let pies = full_info
        .into_iter()
        .map(|pie| build_portfolio_pie_summary(pie, total_current_value))
        .collect();

    GetPortfolioSummaryOutput {
        source: "trading212",
        read_only: true,
        total_current_value_rounded: total_current_value.map(|value| value.round_dp(0)),
        pies,
    }
}

fn build_portfolio_pie_summary(
    pie: FullPieInfo,
    total_current_value: Option<Decimal>,
) -> PortfolioPieSummary {
    let pie_current_value = pie.summary.result.price_avg_value;

    let instruments = pie
        .detail
        .instruments
        .into_iter()
        .map(|instrument| {
            let current_value = instrument.result.price_avg_value;
            let target_share_percent = pie
                .detail
                .settings
                .instrument_shares
                .as_ref()
                .and_then(|shares| shares.get(&instrument.ticker))
                .copied()
                .map(decimal_to_percent);

            PortfolioInstrumentSummary {
                ticker: instrument.ticker,
                current_value_rounded: current_value.map(|value| value.round_dp(0)),
                portfolio_weight_percent: percentage_of(current_value, total_current_value),
                pie_weight_percent: percentage_of(current_value, pie_current_value),
                target_share_percent,
            }
        })
        .collect();

    PortfolioPieSummary {
        name: pie.detail.settings.name,
        current_value_rounded: pie_current_value.map(|value| value.round_dp(0)),
        portfolio_weight_percent: percentage_of(pie_current_value, total_current_value),
        instruments,
    }
}

fn percentage_of(value: Option<Decimal>, total: Option<Decimal>) -> Option<Decimal> {
    let value = value?;
    let total = total?;

    if total.is_zero() {
        return None;
    }

    Some(decimal_to_percent(value / total))
}

fn decimal_to_percent(value: Decimal) -> Decimal {
    (value * Decimal::new(100, 0)).round_dp(2)
}
