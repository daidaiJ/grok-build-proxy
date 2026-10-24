//! `lsp_rename` tool — multi-file symbol rename via the language server.
//!
//! A separate write tool (not an `lsp` operation) on purpose: the six
//! navigation ops ride `is_read_only: true`, and a write path must not drag
//! them into the approval surface. Writes go through the same `FileSystem`
//! resource as the edit tools; the LSP notification happens on the backend
//! directly, so the diagnostics reminder's drain sees the rename's fallout
//! exactly as it does for any edit.

use std::sync::Arc;

use crate::implementations::lsp::LspBackend;
use crate::types::output::ToolOutput;
use crate::types::resources::FileSystem;
use crate::types::tool::{ToolKind, ToolNamespace};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct LspRenameToolOutput(pub String);

impl xai_tool_runtime::ToolOutput for LspRenameToolOutput {}

impl From<LspRenameToolOutput> for ToolOutput {
    fn from(o: LspRenameToolOutput) -> Self {
        ToolOutput::Text(o.0.into())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LspRenameInput {
    #[schemars(description = "Absolute path to a file containing the symbol (used to locate the server and position).")]
    pub file_path: String,
    #[schemars(description = "0-indexed line of the symbol to rename.")]
    pub line: u32,
    #[schemars(description = "0-indexed column of the symbol to rename.")]
    pub character: u32,
    #[schemars(description = "The new name for the symbol.")]
    pub new_name: String,
}

#[derive(Debug, Default)]
pub struct LspRenameTool;

impl crate::types::tool_metadata::ToolMetadata for LspRenameTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Edit
    }

    fn tool_namespace(&self) -> ToolNamespace {
        ToolNamespace::GrokBuild
    }

    fn description_template(&self) -> &str {
        r#"Rename a symbol (function, type, variable, ...) across the workspace via the language server. Use lsp goToDefinition or findReferences first to confirm the position is on the symbol you mean — the position must be on the symbol itself, not a comment or string. This is a write operation: it edits every file the server reports and triggers an approval.
After the rename, verify with the lsp diagnostics operation on the edited files (the result lists them) rather than assuming success."#
    }

    fn emitted_notifications(&self) -> &'static [&'static str] {
        &[
            "LspServerCrashed",
            "LspServerFailed",
            "LspServerReady",
            "LspServerRetrying",
            "LspServerStarting",
        ]
    }
}

impl xai_tool_runtime::Tool for LspRenameTool {
    type Args = LspRenameInput;
    type Output = LspRenameToolOutput;

    fn id(&self) -> xai_tool_protocol::ToolId {
        xai_tool_protocol::ToolId::new("lsp_rename").expect("valid tool id")
    }

    fn description(
        &self,
        _ctx: &::xai_tool_runtime::ListToolsContext,
    ) -> xai_tool_types::ToolDescription {
        xai_tool_types::ToolDescription::new(
            "lsp_rename",
            crate::types::tool_metadata::ToolMetadata::sanitized_description_template(self),
        )
    }

    fn capabilities(&self) -> xai_tool_protocol::ToolCapabilities {
        xai_tool_protocol::ToolCapabilities {
            is_read_only: false,
            tool_scope: Some(xai_tool_protocol::ToolScope::Write),
            ..Default::default()
        }
    }

    #[tracing::instrument(
        name = "tool.lsp_rename",
        skip_all,
        fields(new_name = %input.new_name)
    )]
    async fn run(
        &self,
        ctx: xai_tool_runtime::ToolCallContext,
        input: LspRenameInput,
    ) -> Result<LspRenameToolOutput, xai_tool_runtime::ToolError> {
        use crate::types::tool_metadata::shared_resources;
        let resources = shared_resources(&ctx)?;

        let (backend, fs, cwd) = {
            let res = resources.lock().await;
            let backend = res
                .get::<Arc<dyn LspBackend>>()
                .ok_or_else(|| {
                    xai_tool_runtime::ToolError::custom(
                        "process_manager",
                        "LSP tool is unavailable. Configure ~/.grok/lsp.json or <cwd>/.grok/lsp.json and ensure the language server can start.",
                    )
                })?
                .clone();
            let fs = res.require::<FileSystem>()?.0.clone();
            let cwd = res.require::<crate::types::resources::Cwd>()?.0.clone();
            (backend, fs, cwd)
        };

        if input.new_name.is_empty() {
            return Ok(LspRenameToolOutput("Error: new_name must not be empty.".into()));
        }
        if input.new_name.chars().any(|c| c.is_whitespace() || c == '/' || c == '\\') {
            return Ok(LspRenameToolOutput(format!(
                "Error: new_name {:?} is not a valid symbol name.",
                input.new_name
            )));
        }

        let path = crate::types::resources::resolve_model_path(&cwd, None, &input.file_path);

        let edited = backend
            .rename(&path, input.line, input.character, &input.new_name)
            .await
            .map_err(|e| xai_tool_runtime::ToolError::custom("process_manager", e))?;

        if edited.is_empty() {
            return Ok(LspRenameToolOutput(
                "Rename produced no edits (the symbol may be defined outside the workspace)."
                    .into(),
            ));
        }

        // 先读后写已在 backend 内完成（全批在内存中成型）；这里的写阶段失败
        // 按拍板 D2 报错并列出已写文件，不尝试回滚。
        let mut written: Vec<std::path::PathBuf> = Vec::new();
        for (path, new_content) in &edited {
            if let Err(e) = fs.write_file(path, new_content.as_bytes()).await {
                let listed = written
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n  ");
                return Ok(LspRenameToolOutput(format!(
                    "Error: rename failed partway. {} file(s) already written:\n  {}\nFailed writing {}: {e}",
                    written.len(),
                    listed,
                    path.display()
                )));
            }
            written.push(path.clone());
        }

        // 落盘即通知（读写触发点）：每个改动文件喂回 LSP，诊断回喂不漏。
        for (path, new_content) in &edited {
            backend
                .notify_file_event(path, Some(new_content), crate::implementations::lsp::DiskChangeKind::Changed)
                .await;
        }

        let files_list = edited
            .iter()
            .map(|(p, _)| format!("  {}", p.display()))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(LspRenameToolOutput(format!(
            "Renamed to `{}` across {} file(s):\n{}\nVerify with the lsp diagnostics operation on these files.",
            input.new_name,
            edited.len(),
            files_list
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_tool_runtime::Tool as _;

    /// The write path must never ride the read-only lsp tool: the six
    /// navigation ops stay免审批, rename goes through the edit approval.
    #[test]
    fn rename_tool_is_a_write_tool() {
        let caps = LspRenameTool.capabilities();
        assert!(!caps.is_read_only);
        assert_eq!(
            caps.tool_scope,
            Some(xai_tool_protocol::ToolScope::Write)
        );
        assert_eq!(
            crate::types::tool_metadata::ToolMetadata::kind(&LspRenameTool),
            ToolKind::Edit
        );
    }

    #[test]
    fn rename_tool_id_is_stable() {
        assert_eq!(LspRenameTool.id().as_str(), "lsp_rename");
    }
}
