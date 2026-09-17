//! `/stats` — experimental tool-output compression ledger.

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};
use xai_grok_tools::implementations::output_compression::{
    format_stats_report, is_enabled, stats_for_display,
};

pub struct StatsCommand;

impl SlashCommand for StatsCommand {
    slash_meta! {
        name: "stats",
        description: "Show tool-output compression savings",
        usage: "/stats",
        takes_args: false,
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        let home = xai_grok_config::grok_home();
        let stats = stats_for_display(Some(&home));
        CommandResult::Message(format_stats_report(&stats, is_enabled()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::bundle::BundleState;
    use crate::settings::PagerLocalSnapshot;

    static DEFAULT_BUNDLE_STATE: BundleState = BundleState {
        has_cache: false,
        version: String::new(),
        personas: Vec::new(),
        roles: Vec::new(),
        agents: Vec::new(),
        skills: Vec::new(),
        persona_details: Vec::new(),
        role_details: Vec::new(),
    };

    fn exec_ctx(models: &ModelState) -> CommandExecCtx<'_> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: &DEFAULT_BUNDLE_STATE,
            screen_mode: crate::app::ScreenMode::Minimal,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: PagerLocalSnapshot::default(),
        }
    }

    #[test]
    fn reports_disabled_state_by_default() {
        let models = ModelState::default();
        let mut ctx = exec_ctx(&models);
        match StatsCommand.run(&mut ctx, "") {
            CommandResult::Message(text) => {
                assert!(text.contains("Tool-output compression"));
                assert!(
                    text.contains("enabled: no") || text.contains("enabled: yes"),
                    "{text}"
                );
            }
            other => panic!("expected message, got {other:?}"),
        }
    }
}
