use crate::types::{HookDecision, HookResponse};

pub fn deny(reason: String) -> String {
    let response = HookResponse {
        hook_specific_output: HookDecision {
            hook_event_name: "PreToolUse".to_string(),
            permission_decision: "deny".to_string(),
            permission_decision_reason: reason,
        },
    };
    serde_json::to_string(&response).unwrap_or_default()
}
