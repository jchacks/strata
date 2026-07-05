use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    agent::{AgentMessage, AgentTurn, ChatProvider, ToolCall},
    tools::ToolDefinition,
};

pub struct OpenAiCompatibleChatProvider {
    http: Client,
    api_key: String,
    model: String,
    chat_completions_url: String,
}

impl OpenAiCompatibleChatProvider {
    pub fn openai(api_key: String, model: impl Into<String>) -> Self {
        Self::new(api_key, model, "https://api.openai.com/v1/chat/completions")
    }

    pub fn openrouter(api_key: String, model: impl Into<String>) -> Self {
        Self::new(
            api_key,
            model,
            "https://openrouter.ai/api/v1/chat/completions",
        )
    }

    pub fn new(
        api_key: String,
        model: impl Into<String>,
        chat_completions_url: impl Into<String>,
    ) -> Self {
        Self {
            http: Client::new(),
            api_key,
            model: model.into(),
            chat_completions_url: chat_completions_url.into(),
        }
    }
}

#[async_trait]
impl ChatProvider for OpenAiCompatibleChatProvider {
    async fn next_turn(
        &self,
        messages: &[AgentMessage],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<AgentTurn> {
        tracing::info!(
            model = %self.model,
            url = %self.chat_completions_url,
            message_count = messages.len(),
            tool_count = tools.len(),
            "sending chat completions request"
        );

        let request = ChatRequest {
            model: self.model.clone(),
            messages: messages.iter().map(Message::from).collect(),
            tools: tools.iter().map(ToolDefinitionWire::from).collect(),
            tool_choice: "auto",
        };

        let response = self
            .http
            .post(&self.chat_completions_url)
            .bearer_auth(&self.api_key)
            .header("X-Title", "Strata")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            tracing::warn!(%status, "chat completions request failed");
            anyhow::bail!("chat completions API returned status {status}: {body}");
        }

        tracing::debug!(%status, "chat completions response received");

        let response: ChatResponse = serde_json::from_str(&body)?;
        let message = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("chat completions response did not contain choices"))?
            .message;

        if let Some(tool_calls) = message.tool_calls {
            if !tool_calls.is_empty() {
                let calls = tool_calls
                    .into_iter()
                    .map(|call| {
                        let arguments = if call.function.arguments.trim().is_empty() {
                            json!({})
                        } else {
                            serde_json::from_str(&call.function.arguments)?
                        };

                        Ok(ToolCall {
                            id: call.id,
                            name: call.function.name,
                            arguments,
                        })
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;

                tracing::info!(
                    tool_call_count = calls.len(),
                    "chat model returned tool calls"
                );
                return Ok(AgentTurn::ToolCalls(calls));
            }
        }

        tracing::info!("chat model returned final content");
        Ok(AgentTurn::Final(message.content.unwrap_or_default()))
    }
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    tools: Vec<ToolDefinitionWire>,
    tool_choice: &'static str,
}

#[derive(Debug, Serialize)]
struct ToolDefinitionWire {
    #[serde(rename = "type")]
    kind: ToolType,
    function: FunctionDefinition,
}

impl From<&ToolDefinition> for ToolDefinitionWire {
    fn from(definition: &ToolDefinition) -> Self {
        Self {
            kind: ToolType::Function,
            function: FunctionDefinition {
                name: definition.name.clone(),
                description: definition.description.clone(),
                parameters: definition.input_schema.clone(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum ToolType {
    Function,
}

#[derive(Debug, Serialize)]
struct FunctionDefinition {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Debug, Serialize)]
#[serde(tag = "role")]
enum Message {
    #[serde(rename = "system")]
    System { content: String },
    #[serde(rename = "user")]
    User { content: String },
    #[serde(rename = "assistant")]
    Assistant {
        content: Option<String>,
        tool_calls: Vec<AssistantToolCall>,
    },
    #[serde(rename = "tool")]
    Tool {
        tool_call_id: String,
        content: String,
    },
}

impl From<&AgentMessage> for Message {
    fn from(message: &AgentMessage) -> Self {
        match message {
            AgentMessage::System(content) => Self::System {
                content: content.clone(),
            },
            AgentMessage::User(content) => Self::User {
                content: content.clone(),
            },
            AgentMessage::Assistant {
                content,
                tool_calls,
            } => Self::Assistant {
                content: content.clone(),
                tool_calls: tool_calls.iter().map(AssistantToolCall::from).collect(),
            },
            AgentMessage::Tool {
                tool_call_id,
                content,
            } => Self::Tool {
                tool_call_id: tool_call_id.clone(),
                content: content.to_string(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct AssistantToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: ToolType,
    function: AssistantFunctionCall,
}

impl From<&ToolCall> for AssistantToolCall {
    fn from(tool_call: &ToolCall) -> Self {
        Self {
            id: tool_call.id.clone(),
            kind: ToolType::Function,
            function: AssistantFunctionCall {
                name: tool_call.name.clone(),
                arguments: tool_call.arguments.to_string(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct AssistantFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<ResponseToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ResponseToolCall {
    id: String,
    function: ResponseFunctionCall,
}

#[derive(Debug, Deserialize)]
struct ResponseFunctionCall {
    name: String,
    arguments: String,
}
