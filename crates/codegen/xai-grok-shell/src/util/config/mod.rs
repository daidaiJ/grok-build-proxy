// `McpOAuthConfig` / `McpOAuthConfigMap` are re-exported via `mcp` (see `mcp.rs`)

mod announcements;
mod campaigns;
mod consent;
mod hints;
mod load;
// LOCAL: /auth 面板的 [model.<id>] 写回（toml_edit 定点编辑 + config 写守卫）
mod model_entry_writes;
mod mcp;
mod mcp_reenable;
mod permissions;
mod persist;
mod resolve;
mod settings_writes;
mod tips;
mod worktree;

pub use announcements::*;
pub use campaigns::{
    CampaignModelsDefault, campaign_driven_models_default, persist_models_default,
    sync_campaign_fields,
};
pub use consent::*;
pub use hints::*;
pub use load::*;
pub use mcp::*;
pub(crate) use mcp_reenable::{McpDefinitionIndex, needs_definition_scan};
pub use model_entry_writes::{ModelEntryDraft, write_model_entries};
// LOCAL（视觉桥 D8）：目录能力位回填（crate 内部消费，不走公开面板面）。
pub(crate) use model_entry_writes::backfill_model_capabilities;
pub use permissions::*;
pub use persist::*;
pub use resolve::*;
pub use settings_writes::*;
pub use tips::*;
pub use worktree::*;
pub use xai_grok_config::effective_config::{
    EffectiveConfigLayers, load_effective_config, load_effective_config_with_layers,
    remote_campaigns_from_settings, set_remote_campaigns_from_settings,
};
pub use xai_grok_config::load_effective_config_disk_only;
// These types live in `xai-grok-config`; the re-export keeps `crate::util::config::{RemoteSettings, GoalRoleModel}` working
pub use xai_grok_config_types::{
    CampaignOverride, ConsentGate, ContextualHintsRemote, DisplayRefreshSettings,
    DoomLoopRecoverySettings, GoalRoleModel, LongReasoningReminderSettings, RemoteSettings,
    WorktreeAutoGcSettings, WorktreeKindMaxAge, deserialize_tolerant,
};
