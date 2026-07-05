use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tools::{ToolDefinition, ToolRegistry};

pub mod openai_compatible;

const SYSTEM_PROMPT: &str = include_str!("prompts/system.txt");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentMessage {
    System(String),
    User(String),
    Assistant {
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        tool_call_id: String,
        content: Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone)]
pub enum AgentTurn {
    Final(String),
    ToolCalls(Vec<ToolCall>),
}

#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn next_turn(
        &self,
        messages: &[AgentMessage],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<AgentTurn>;
}

pub async fn run_tool_loop<P>(
    provider: &P,
    registry: &ToolRegistry,
    user_prompt: String,
) -> anyhow::Result<String>
where
    P: ChatProvider,
{
    let tools = registry.llm_definitions();
    tracing::info!(tool_count = tools.len(), "starting agent tool loop");

    let mut messages = vec![
        AgentMessage::System(SYSTEM_PROMPT.to_string()),
        AgentMessage::User(user_prompt),
    ];

    for turn_index in 0..8 {
        tracing::debug!(
            turn_index,
            message_count = messages.len(),
            "requesting next agent turn"
        );

        match provider.next_turn(&messages, &tools).await? {
            AgentTurn::Final(answer) => {
                tracing::info!(turn_index, "agent returned final answer");
                return Ok(answer);
            }
            AgentTurn::ToolCalls(tool_calls) => {
                tracing::info!(
                    turn_index,
                    tool_call_count = tool_calls.len(),
                    "agent requested tool calls"
                );
                messages.push(AgentMessage::Assistant {
                    content: None,
                    tool_calls: tool_calls.clone(),
                });

                for tool_call in tool_calls {
                    tracing::info!(
                        tool_call_id = %tool_call.id,
                        tool_name = %tool_call.name,
                        "executing requested tool call"
                    );

                    let result = registry
                        .call(&tool_call.name, tool_call.arguments.clone())
                        .await;

                    messages.push(AgentMessage::Tool {
                        tool_call_id: tool_call.id,
                        content: result,
                    });
                }
            }
        }
    }

    anyhow::bail!("agent loop exceeded maximum tool-call turns")
}
