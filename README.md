# Strata

Strata is a local portfolio analysis project for exploring ETF holdings, portfolio structure, and exposure.

It combines a deterministic Rust core with typed tools that can be called by an agent. The core is responsible for collecting and shaping portfolio facts; the agent is used to ask questions and explain structured outputs.

Strata is not, and will not become, an automatic trading bot.

> [!IMPORTANT]
> Strata is a personal portfolio analysis and research tool. It does not provide financial, investment, tax, or legal advice. Outputs are informational only and may be incomplete or incorrect. You are responsible for verifying results and making your own investment decisions.
>
> Strata does not place trades, modify Trading 212 pies, or perform automatic rebalancing.

## Overview

The project is currently focused on getting the core tool-calling loop working with real portfolio data from Trading 212.

The broader direction is to make portfolio data easier to inspect and reason about: holdings, weights, overlap, exposure, policy alignment, and related explanations. The exact shape of those features is intentionally still evolving.

The main boundary is simple: Rust tools produce facts; the agent explains them.

## Current status

Implemented:

- Trading 212 pie summary/detail fetches
- local cache for Trading 212 pie data
- `get_portfolio_summary` typed tool
- OpenAI-compatible chat-completions adapter
- simple single-prompt agent loop

Not implemented yet:

- follow-up chat sessions
- canonical instrument metadata
- ETF holdings ingestion
- look-through exposure calculations
- policy checks

## Current tool

### `get_portfolio_summary`

Returns a read-only Trading 212 portfolio summary
 containing:

- pie names
- instrument tickers
- rounded current values
- portfolio weights
- pie weights
- target shares when available

This tool does not compute ETF look-through exposure.

## Configuration

Create a `.env` file:

```env
TRADING212_API_KEY=...
TRADING212_API_SECRET=...

OPENROUTER_API_KEY=...
LLM_MODEL=nvidia/nemotron-3-ultra-550b-a55b:free

# Or use an OpenAI API key instead:
# OPENAI_API_KEY=...
# LLM_MODEL=gpt-4.1-mini
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
