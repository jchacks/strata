use anyhow::Context;

use crate::{
    agent::{openai_compatible::OpenAiCompatibleChatProvider, run_tool_loop},
    tools::{ToolRegistry, trading212::Trading212Toolset},
    trading212::Trading212Client,
};

mod agent;
mod tools;
mod trading212;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let api_key = std::env::var("TRADING212_API_KEY").context("TRADING212_API_KEY is not set")?;
    let api_secret =
        std::env::var("TRADING212_API_SECRET").context("TRADING212_API_SECRET is not set")?;
    let llm_api_key = std::env::var("OPENROUTER_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .context("OPENROUTER_API_KEY or OPENAI_API_KEY is not set")?;

    let llm_model = std::env::var("LLM_MODEL")
        .unwrap_or_else(|_| "meta-llama/llama-3.1-8b-instruct:free".to_string());

    let t212_client =
        Trading212Client::new("https://live.trading212.com/api/v0/", api_key, api_secret)
            .context("failed to create Trading 212 client")?;

    let mut registry = ToolRegistry::new();
    let t212_toolset = Trading212Toolset::new(t212_client, "cache");
    t212_toolset.register_tools(&mut registry);

    let prompt = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let prompt = if prompt.trim().is_empty() {
        "What Trading 212 pies do I have? Summarize them briefly.".to_string()
    } else {
        prompt
    };

    let provider = if std::env::var("OPENROUTER_API_KEY").is_ok() {
        OpenAiCompatibleChatProvider::openrouter(llm_api_key, llm_model)
    } else {
        OpenAiCompatibleChatProvider::openai(llm_api_key, llm_model)
    };
    let answer = run_tool_loop(&provider, &registry, prompt).await?;

    println!("{answer}");

    Ok(())
}
