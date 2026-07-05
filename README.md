# Strata

Strata is a local portfolio analysis project for exploring holdings, portfolio structure, and exposure.

It combines local portfolio data collection with agent-readable tools. Broker integrations are just one source of data: the current implementation uses Trading 212 Pies for testing against real portfolio data, but the design is intended to support other brokers and data sources.

Strata is not, and will not become, an automatic trading bot.

> [!IMPORTANT]
> Strata is a personal portfolio analysis and research tool. It does not provide financial, investment, tax, or legal advice. Outputs are informational only and may be incomplete or incorrect. You are responsible for verifying results and making your own investment decisions.
>
> Strata does not place trades, modify broker accounts, or perform automatic rebalancing.

## Overview

The project is currently focused on making real portfolio data available to an agent in a controlled, read-only way. Trading 212 Pies are the first integration because they provide a practical test case, not because Strata is intended to be tied to a single broker.

The broader direction is to make portfolio data easier to inspect and reason about: holdings, weights, overlap, exposure, policy alignment, and related explanations. The exact shape of those features is intentionally still evolving.

The main boundary is simple: integrations provide portfolio facts; the agent explains them.

## Current status

Implemented:

- Trading 212 pie summary/detail fetches as the first broker integration
- local cache for Trading 212 pie data
- `get_portfolio_summary` portfolio summary tool
- OpenAI-compatible chat-completions adapter
- simple single-prompt agent loop

Not implemented yet:

- follow-up chat sessions
- canonical instrument metadata
- ETF holdings ingestion
- look-through exposure calculations
- parallel research sub-agents
- policy checks

## Current tool

### `get_portfolio_summary`

Returns a read-only portfolio summary from the current Trading 212 integration containing:

- pie names
- instrument tickers
- rounded current values
- portfolio weights
- pie weights
- target shares when available

This tool does not compute ETF look-through exposure.

## Agent direction

The agent layer is intended to sit on top of read-only portfolio tools rather than become the source of portfolio truth.

One planned workflow is to split broader questions into smaller research tasks that can run in parallel. For example, a portfolio-level question could dispatch separate sub-agents to inspect individual ETFs, compare index methodologies, summarize fees and domicile, or collect issuer facts. Each sub-agent would return a concise, source-aware report that the main agent can combine with verified portfolio facts.

Useful roles may include:

- instrument lookup: map broker tickers to canonical identifiers such as ISINs and exchange tickers
- fund research: summarize ETF objectives, index tracked, fees, domicile, replication method, and distribution policy
- exposure analysis: compare overlapping holdings, sectors, regions, currencies, and factor tilts once holdings data is available
- policy review: check a portfolio against user-defined constraints without making trade decisions

These agents should explain and summarize. They should not place trades, mutate broker state, or treat generated text as the source of truth.

## Configuration

Create a `.env` file:

```env
TRADING212_API_KEY=...
TRADING212_API_SECRET=...

OPENROUTER_API_KEY=...
MODEL_NAME=nvidia/nemotron-3-ultra-550b-a55b:free

# Or use an OpenAI API key instead:
# OPENAI_API_KEY=...
# MODEL_NAME=gpt-4.1-mini
```

`.env` and `cache/` are ignored by git.

## Run

```bash
cargo run -- "What Trading 212 pies do I have? Summarize them briefly."
```

With logs:

```bash
RUST_LOG=info cargo run -- "Use the portfolio summary tool and summarize my holdings."
```

## Cache

Trading 212 pie data is cached at:

```text
cache/t212/full_pie_info.json
```

Delete this file to force a fresh Trading 212 fetch.
