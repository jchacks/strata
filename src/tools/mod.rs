use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

pub mod trading212;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[async_trait]
pub trait TypedTool: Send + Sync {
    type Input: DeserializeOwned + JsonSchema + Send + Sync + 'static;
    type Output: Serialize + JsonSchema + Send + Sync + 'static;

    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;

    async fn call_typed(&self, input: Self::Input) -> anyhow::Result<Self::Output>;
}

#[async_trait]
pub trait ErasedTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> Value;
    fn output_schema(&self) -> Value;

    async fn call_json(&self, args: Value) -> Value;
}

#[async_trait]
impl<T> ErasedTool for T
where
    T: TypedTool + Send + Sync,
{
    fn name(&self) -> &'static str {
        TypedTool::name(self)
    }

    fn description(&self) -> &'static str {
        TypedTool::description(self)
    }

    fn input_schema(&self) -> Value {
        serde_json::to_value(schema_for!(T::Input)).expect("tool input schema should serialize")
    }

    fn output_schema(&self) -> Value {
        serde_json::to_value(schema_for!(T::Output)).expect("tool output schema should serialize")
    }

    async fn call_json(&self, args: Value) -> Value {
        let input = match serde_json::from_value::<T::Input>(args) {
            Ok(input) => input,
            Err(err) => {
                return json!({
                    "ok": false,
                    "error": {
                        "code": "invalid_arguments",
                        "message": err.to_string(),
                    }
                });
            }
        };

        match self.call_typed(input).await {
            Ok(output) => json!({
                "ok": true,
                "data": output,
            }),
            Err(err) => json!({
                "ok": false,
                "error": {
                    "code": "tool_execution_failed",
                    "message": err.to_string(),
                }
            }),
        }
    }
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<&'static str, Arc<dyn ErasedTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T>(&mut self, tool: T)
    where
        T: ErasedTool + 'static,
    {
        let name = ErasedTool::name(&tool);
        tracing::debug!(tool_name = name, "registering tool");
        self.tools.insert(name, Arc::new(tool));
    }

    pub fn llm_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                input_schema: tool.input_schema(),
            })
            .collect()
    }

    pub fn definitions_json(&self) -> Vec<Value> {
        self.tools
            .values()
            .map(|tool| {
                json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "input_schema": tool.input_schema(),
                    "output_schema": tool.output_schema(),
                })
            })
            .collect()
    }

    pub async fn call(&self, name: &str, args: Value) -> Value {
        tracing::info!(tool_name = name, "calling tool");

        match self.tools.get(name) {
            Some(tool) => {
                let result = tool.call_json(args).await;
                tracing::info!(tool_name = name, "tool call completed");
                result
            }
            None => {
                tracing::warn!(tool_name = name, "unknown tool requested");
                json!({
                    "ok": false,
                    "error": {
                        "code": "unknown_tool",
                        "message": format!("Unknown tool: {name}"),
                    }
                })
            }
        }
    }
}
