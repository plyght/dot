use super::Tool;

pub struct BatchTool;

impl Tool for BatchTool {
    fn name(&self) -> &str {
        "batch"
    }

    fn description(&self) -> &str {
        "Run multiple tool calls in parallel and return all results. Use when you need to call several independent tools at once to save round-trips."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "invocations": {
                    "type": "array",
                    "description": "List of tool invocations to execute",
                    "items": {
                        "type": "object",
                        "properties": {
                            "tool_name": { "type": "string", "description": "Name of the tool to call" },
                            "input": { "type": "object", "description": "Input parameters for the tool" }
                        },
                        "required": ["tool_name", "input"]
                    }
                }
            },
            "required": ["invocations"]
        })
    }

    fn execute(&self, _input: serde_json::Value) -> anyhow::Result<String> {
        anyhow::bail!("batch is a virtual tool handled by the agent loop")
    }
}
