//! `expand_output` — pull-style retrieval of a CCR-stored original.

use crate::types::output::ToolOutput;
use crate::types::tool::{ToolKind, ToolNamespace};
use crate::util::truncate::estimate_tokens;

use super::runtime::record_retrieve;
use super::store;

pub const EXPAND_OUTPUT_TOOL_NAME: &str = "expand_output";

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
pub struct ExpandOutputInput {
    /// 24-hex CCR hash from a `<<ccr:HASH>>` marker (alias: `tool_use_id`).
    #[serde(alias = "tool_use_id")]
    pub hash: String,
}

#[derive(Debug, Default)]
pub struct ExpandOutputTool;

impl crate::types::tool_metadata::ToolMetadata for ExpandOutputTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Read
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        "Retrieve the original uncompressed tool output stored by experimental \
         tool-output compression. Pass the 24-character hex hash from a `<<ccr:HASH>>` \
         marker. No-op unless `[tool_output_compression]` is enabled."
    }
}

impl xai_tool_runtime::Tool for ExpandOutputTool {
    type Args = ExpandOutputInput;
    type Output = ToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new(EXPAND_OUTPUT_TOOL_NAME).expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            EXPAND_OUTPUT_TOOL_NAME,
            crate::types::tool_metadata::ToolMetadata::sanitized_description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: true,
            tool_scope: Some(xai_tool_protocol::ToolScope::Read),
            ..Default::default()
        }
    }

    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: ExpandOutputInput,
    ) -> Result<ToolOutput, xai_tool_runtime::ToolError> {
        let hash = input
            .hash
            .trim()
            .trim_matches(|c| c == '<' || c == '>' || c == ':');
        let hash = hash.strip_prefix("ccr:").unwrap_or(hash).trim();
        let rt = match crate::types::tool_metadata::shared_resources(&ctx) {
            Ok(res) => res
                .lock()
                .await
                .get::<super::SessionCompressionPolicy>()
                .cloned()
                .map(|p| p.0)
                .unwrap_or_else(super::runtime::current_runtime),
            Err(_) => super::runtime::current_runtime(),
        };
        match store::get(hash, &rt) {
            Some(original) => {
                record_retrieve(estimate_tokens(&original) as u64);
                Ok(ToolOutput::Text(original.into()))
            }
            None => Ok(ToolOutput::Text(
                format!(
                    "No stored original for hash `{hash}`. It may have expired, \
                     compression is off, or the hash is wrong."
                )
                .into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_id_matches_constant() {
        assert_eq!(
            xai_tool_runtime::Tool::id(&ExpandOutputTool).to_string(),
            EXPAND_OUTPUT_TOOL_NAME
        );
    }
}
