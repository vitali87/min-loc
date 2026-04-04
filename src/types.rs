use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct HookInput {
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub cwd: Option<String>,
}

#[derive(Debug)]
pub enum ParsedToolInput {
    Write {
        file_path: String,
        content: String,
    },
    Edit {
        file_path: String,
        new_string: String,
    },
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub lang: Option<String>,
    pub test_cmd: Option<String>,
    pub module_name: Option<String>,
    pub timeout: Option<u32>,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct LOCStats {
    pub total: u32,
    pub non_blank: u32,
    pub non_import: u32,
}

#[derive(Debug, Serialize)]
pub struct HookResponse {
    #[serde(rename = "hookSpecificOutput")]
    pub hook_specific_output: HookDecision,
}

#[derive(Debug, Serialize)]
pub struct HookDecision {
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,
    #[serde(rename = "permissionDecision")]
    pub permission_decision: String,
    #[serde(rename = "permissionDecisionReason")]
    pub permission_decision_reason: String,
}
