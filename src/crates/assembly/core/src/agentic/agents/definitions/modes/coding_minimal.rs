//! Explicit opt-in coding mode with a closed, minimal tool profile.

use crate::agentic::agents::{
    coding_minimal_mode_tool_exposure_overrides, coding_minimal_mode_tools,
    shared_coding_mode_user_context_policy, Agent, AgentToolPolicyOverrides, UserContextPolicy,
    CODING_MINIMAL_MODE_ID, CODING_MINIMAL_MODE_NAME, CODING_MINIMAL_MODE_PROMPT_TEMPLATE,
};
use async_trait::async_trait;

pub struct CodingMinimalMode {
    default_tools: Vec<String>,
    tool_exposure_overrides: AgentToolPolicyOverrides,
}

impl Default for CodingMinimalMode {
    fn default() -> Self {
        Self::new()
    }
}

impl CodingMinimalMode {
    pub fn new() -> Self {
        Self {
            default_tools: coding_minimal_mode_tools(),
            tool_exposure_overrides: coding_minimal_mode_tool_exposure_overrides(),
        }
    }
}

#[async_trait]
impl Agent for CodingMinimalMode {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> &str {
        CODING_MINIMAL_MODE_ID
    }

    fn name(&self) -> &str {
        CODING_MINIMAL_MODE_NAME
    }

    fn description(&self) -> &str {
        "Coding mode with a closed four-tool baseline and contextual command controls"
    }

    fn prompt_template_name(&self, _model_name: Option<&str>) -> &str {
        CODING_MINIMAL_MODE_PROMPT_TEMPLATE
    }

    fn default_tools(&self) -> Vec<String> {
        self.default_tools.clone()
    }

    fn tool_exposure_overrides(&self) -> &AgentToolPolicyOverrides {
        &self.tool_exposure_overrides
    }

    fn user_context_policy(&self) -> UserContextPolicy {
        shared_coding_mode_user_context_policy()
    }

    fn is_readonly(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::CodingMinimalMode;
    use crate::agentic::agents::{
        get_embedded_prompt, shared_coding_mode_user_context_policy, Agent, CODING_MINIMAL_MODE_ID,
        CODING_MINIMAL_MODE_NAME, CODING_MINIMAL_MODE_PROMPT_TEMPLATE,
    };
    use crate::agentic::tools::framework::ToolExposure;

    #[test]
    fn coding_minimal_identity_prompt_context_and_tools_are_stable() {
        let mode = CodingMinimalMode::new();

        assert_eq!(mode.id(), CODING_MINIMAL_MODE_ID);
        assert_eq!(mode.name(), CODING_MINIMAL_MODE_NAME);
        assert_eq!(
            mode.prompt_template_name(None),
            CODING_MINIMAL_MODE_PROMPT_TEMPLATE
        );
        assert_eq!(
            mode.user_context_policy(),
            shared_coding_mode_user_context_policy()
        );
        assert_eq!(
            mode.default_tools(),
            [
                "Read",
                "Edit",
                "Write",
                "ExecCommand",
                "WriteStdin",
                "ExecControl",
            ]
            .map(str::to_string)
        );
        assert!(mode
            .tool_exposure_overrides()
            .values()
            .all(|exposure| *exposure == ToolExposure::Direct));
    }

    #[test]
    fn coding_minimal_prompt_mentions_no_unavailable_tool_identifiers() {
        let prompt = get_embedded_prompt(CODING_MINIMAL_MODE_PROMPT_TEMPLATE)
            .expect("coding minimal prompt must be embedded");
        let unavailable = [
            "LS",
            "Glob",
            "Grep",
            "Delete",
            "Git",
            "WriteStdin",
            "ExecControl",
            "Task",
            "AgentWait",
            "ListModels",
            "Skill",
            "AskUserQuestion",
            "TodoWrite",
            "WebSearch",
            "WebFetch",
            "GetToolSpec",
            "CallDeferredTool",
            "ControlHub",
        ];

        for identifier in unavailable {
            assert!(
                !prompt
                    .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
                    .any(|token| token == identifier),
                "minimal prompt must not mention unavailable tool identifier {identifier}"
            );
        }
    }
}
