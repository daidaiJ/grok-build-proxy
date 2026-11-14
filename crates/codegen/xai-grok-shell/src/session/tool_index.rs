//! Builds a BM25 index over registered MCP tools and searches it.
//! The index is rebuilt on each search call (sub-millisecond for tens to low hundreds of tools).

use std::sync::Arc;

use std::sync::Mutex;

use bm25::{Language, SearchEngineBuilder};
use xai_grok_tools::types::tool_index::{
    SearchSnapshot, ServerSummary, ToolSearchIndex, ToolSearchResult,
};

use super::mcp_servers::MCP_TOOL_NAME_DELIMITER;

/// LOCAL(token-economy): built-in tools that stay out of the per-request declaration and are
/// declared on demand through `search_tool` / `use_tool`.
///
/// Zero calls in a 30-day window (112 sessions, ~9k tool calls) except `scheduler_*`, which the
/// user explicitly deferred on 2026-11-14. The list is static so the declared tool surface stays
/// byte-identical across turns; removing a name restores that tool as a direct declaration.
pub(crate) const DEFERRED_BUILTIN_TOOLS: &[&str] = &[
    "workflow",
    "reference_to_video",
    "send_feedback",
    "lsp",
    "image_gen",
    "image_edit",
    "image_to_video",
    "scheduler_create",
    "scheduler_delete",
    "scheduler_list",
];

/// Snapshot entries for deferred built-ins, using the same shape as MCP tools so the BM25 index
/// can search both with one corpus. `server_name` is the fixed search group shown to the model.
pub(crate) fn deferred_builtin_metadata(
    defs: &[xai_grok_sampling_types::ToolDefinition],
) -> Vec<ToolMetadata> {
    defs.iter()
        .filter(|def| DEFERRED_BUILTIN_TOOLS.contains(&def.function.name.as_str()))
        .map(|def| ToolMetadata {
            qualified_name: def.function.name.clone(),
            server_name: "built-in".to_string(),
            tool_name: def.function.name.clone(),
            description: def.function.description.clone().unwrap_or_default(),
            parameters: extract_parameter_names(&def.function.parameters),
            input_schema: def.function.parameters.clone(),
        })
        .collect()
}

/// Handles `__` (qualified-name delimiter), `_` (snake_case), `-` (kebab-case), and camelCase / PascalCase boundaries.
fn split_identifier(s: &str) -> Vec<&str> {
    let mut words: Vec<&str> = Vec::new();
    for part in s
        .split("__")
        .flat_map(|p| p.split('_'))
        .flat_map(|p| p.split('-'))
    {
        if part.is_empty() {
            continue;
        }
        // Split on camelCase boundaries: a lowercase to uppercase transition
        let bytes = part.as_bytes();
        let mut start = 0;
        for i in 1..bytes.len() {
            let Some(prev) = i.checked_sub(1).and_then(|j| bytes.get(j)) else {
                continue;
            };
            let Some(cur) = bytes.get(i) else {
                break;
            };
            if prev.is_ascii_lowercase() && cur.is_ascii_uppercase() {
                if let Some(word) = part.get(start..i) {
                    words.push(word);
                }
                start = i;
            }
        }
        if let Some(word) = part.get(start..) {
            words.push(word);
        }
    }
    words
}

/// If the query contains `__`, `_`, `-`, or camelCase, the split components are appended so BM25 can match on individual parts.
/// Plain natural-language queries pass through unchanged.
fn normalize_query(query: &str) -> String {
    let needs_split = query.contains("__")
        || query.contains('_')
        || query.contains('-')
        || query.as_bytes().windows(2).any(|w| {
            let [a, b] = w else {
                return false;
            };
            a.is_ascii_lowercase() && b.is_ascii_uppercase()
        });
    if !needs_split {
        return query.to_owned();
    }
    let extra: Vec<&str> = query
        .split_whitespace()
        .flat_map(split_identifier)
        .collect();
    if extra.is_empty() {
        return query.to_owned();
    }
    format!("{query} {}", extra.join(" "))
}

/// Metadata for a single MCP tool, used to build BM25 documents.
#[derive(Debug, Clone)]
pub(crate) struct ToolMetadata {
    /// Canonical name (e.g., `"linear__save_issue"` or a managed gateway `{connector_id}__{tool_id}`).
    pub qualified_name: String,
    /// Server, source, or grouping name (e.g., `"linear"`).
    pub server_name: String,
    /// Search/display tool name.
    pub tool_name: String,
    pub description: String,
    /// Parameter names from input schema.
    pub parameters: Vec<String>,
    /// Full JSON Schema for the tool's input parameters.
    pub input_schema: serde_json::Value,
}

impl ToolMetadata {
    /// The document text is `{server_name} {tool_name} {description} {parameter_names}`.
    /// The server, tool, and parameter names are also split into component words (snake_case, camelCase, kebab-case) and appended.
    /// Exact canonical-name lookup is handled before BM25 ranking.
    fn to_document(&self) -> String {
        let params = self.parameters.join(" ");
        let doc = format!(
            "{} {} {} {}",
            self.server_name, self.tool_name, self.description, params
        );
        // Decompose identifiers into component words so BM25 can match individual parts
        // e.g. "SearchDashboards" becomes "Search Dashboards", "grafana-ai" becomes "grafana ai".
        let extra: String = [self.server_name.as_str(), self.tool_name.as_str()]
            .iter()
            .flat_map(|s| split_identifier(s))
            .chain(self.parameters.iter().flat_map(|p| split_identifier(p)))
            .collect::<Vec<_>>()
            .join(" ");
        format!("{doc} {extra}")
    }
}

/// Per-server metadata (name and optional description from the MCP initialize handshake's `instructions` field).
/// Tool count is derived from `ToolMetadataSnapshot::tools` at read time.
/// Disabled tools are unregistered from the bridge before the snapshot is rebuilt, so the count never includes them.
#[derive(Debug, Clone)]
pub(crate) struct ServerMetadata {
    pub name: String,
    pub description: Option<String>,
}

/// A snapshot of MCP tool metadata, shared between the session and the search index.
///
/// Updated when MCP tools are registered or re-initialized.
#[derive(Debug, Clone, Default)]
pub(crate) struct ToolMetadataSnapshot {
    pub tools: Vec<ToolMetadata>,
    pub servers: Vec<ServerMetadata>,
    pub mcp_initialized: bool,
    /// Built-ins that are only declared on demand. Kept outside `tools` so MCP refreshes, which
    /// replace the MCP tool list wholesale, never drop them.
    pub deferred_builtins: Vec<ToolMetadata>,
}

/// Holds a shared snapshot of MCP tool metadata behind a `std::sync::Mutex`.
/// The lock is held only to clone the snapshot (fast, no I/O).
/// `search_snapshot()` is a sync trait method called from async context.
pub(crate) struct Bm25ToolSearchIndex {
    snapshot: Arc<Mutex<ToolMetadataSnapshot>>,
}

impl Bm25ToolSearchIndex {
    pub(crate) fn new(snapshot: Arc<Mutex<ToolMetadataSnapshot>>) -> Self {
        Self { snapshot }
    }
}

impl ToolSearchIndex for Bm25ToolSearchIndex {
    fn search_snapshot(&self, query: &str, limit: usize) -> SearchSnapshot {
        let snapshot = self.snapshot.lock().unwrap().clone();

        let is_ready = snapshot.mcp_initialized;
        let mut tools = snapshot.tools;
        tools.extend(snapshot.deferred_builtins);
        let total_hidden_tools = tools.len();

        if tools.is_empty() {
            return SearchSnapshot {
                results: Vec::new(),
                total_hidden_tools,
                is_ready,
            };
        }

        // Fast path: exact match on qualified name or bare tool name.
        // When the model already knows the tool name (e.g. "grafana-ai__SearchDashboards" or "SearchDashboards"), skip BM25 entirely.
        let query_lower = query.trim().to_lowercase();
        if let Some(exact) = tools.iter().find(|t| {
            t.qualified_name.to_lowercase() == query_lower
                || t.tool_name.to_lowercase() == query_lower
        }) {
            return SearchSnapshot {
                results: vec![ToolSearchResult {
                    tool_name: exact.qualified_name.clone(),
                    server_name: exact.server_name.clone(),
                    description: exact.description.clone(),
                    score: 1.0,
                    parameters: exact.parameters.clone(),
                    input_schema: exact.input_schema.clone(),
                }],
                total_hidden_tools,
                is_ready,
            };
        }

        let documents: Vec<String> = tools.iter().map(|t| t.to_document()).collect();

        let search_engine =
            SearchEngineBuilder::<u32>::with_corpus(Language::English, documents).build();

        let normalized = normalize_query(query);
        let bm25_results = search_engine.search(&normalized, limit);

        let results = bm25_results
            .into_iter()
            .filter_map(|sr| {
                let meta = tools.get(sr.document.id as usize)?;
                Some(ToolSearchResult {
                    tool_name: meta.qualified_name.clone(),
                    server_name: meta.server_name.clone(),
                    description: meta.description.clone(),
                    score: sr.score,
                    parameters: meta.parameters.clone(),
                    input_schema: meta.input_schema.clone(),
                })
            })
            .collect();

        SearchSnapshot {
            results,
            total_hidden_tools,
            is_ready,
        }
    }

    fn is_deferred_builtin(&self, tool_name: &str) -> bool {
        self.snapshot
            .lock()
            .unwrap()
            .deferred_builtins
            .iter()
            .any(|tool| tool.qualified_name == tool_name)
    }

    fn list_server_summaries(&self) -> Vec<ServerSummary> {
        let (tools_snapshot, servers) = {
            let snapshot = self.snapshot.lock().unwrap();
            let tools: Vec<(String, String)> = snapshot
                .tools
                .iter()
                .map(|t| (t.server_name.clone(), t.tool_name.clone()))
                .collect();
            let srvs: Vec<ServerMetadata> = snapshot.servers.clone();
            (tools, srvs)
        };

        let mut map: std::collections::BTreeMap<String, (usize, Option<String>, Vec<String>)> =
            std::collections::BTreeMap::new();
        for (server, tool) in tools_snapshot {
            let (count, _desc, names) = map.entry(server).or_insert((0, None, Vec::new()));
            *count += 1;
            names.push(tool);
        }
        for s in servers {
            let (_count, desc, _names) = map.entry(s.name).or_insert((0, None, Vec::new()));
            *desc = s.description;
        }

        map.into_iter()
            .map(|(name, (tool_count, description, mut tool_names))| {
                tool_names.sort_unstable();
                ServerSummary {
                    name,
                    description,
                    tool_count,
                    tool_names,
                }
            })
            .collect()
    }
}

pub(crate) fn extract_parameter_names(schema: &serde_json::Value) -> Vec<String> {
    schema
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default()
}

/// Split a qualified MCP tool name (`"server__tool"`) into `(server, tool)`.
pub(crate) fn split_qualified_name(qualified: &str) -> (&str, &str) {
    qualified
        .split_once(MCP_TOOL_NAME_DELIMITER)
        .unwrap_or(("", qualified))
}

#[cfg(test)]
#[path = "tool_index_tests.rs"]
mod tests;
