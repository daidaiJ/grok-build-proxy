//! LOCAL: `/lang` 切换 TUI 界面文案语言（中文/English），默认中文。
//!
//! 切换是进程级全局状态；分发层检测到语言变化后会重建注册表 triggers，
//! 让斜杠菜单、命令面板、ghost 补全的描述文本立即换语言。

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};
use crate::slash::i18n::{self, Lang};

pub struct LangCommand;

impl SlashCommand for LangCommand {
    slash_meta! {
        name: "lang",
        description: "Switch UI language (Chinese/English)",
        usage: "/lang [zh|en]",
        takes_args: true,
        arg_placeholder: "zh|en",
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let arg = args.trim().to_ascii_lowercase();
        let target = match arg.as_str() {
            "" => // 无参数：在中英之间切换
                match i18n::current_lang() {
                    Lang::Zh => Lang::En,
                    Lang::En => Lang::Zh,
                },
            "zh" | "中文" | "chinese" => Lang::Zh,
            "en" | "english" => Lang::En,
            other => {
                return CommandResult::Error(format!(
                    "{}: /lang [zh|en]（未知参数 “{other}”）",
                    i18n::tr("Switch UI language (Chinese/English)")
                ));
            }
        };
        if target != i18n::current_lang() {
            i18n::set_lang(target);
        }
        CommandResult::Message(match target {
            Lang::Zh => "界面语言已切换为中文".to_string(),
            Lang::En => "UI language switched to English".to_string(),
        })
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
    fn switches_to_explicit_language() {
        let _guard = crate::slash::i18n::test_sync::LANG_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let before = i18n::current_lang();
        let models = ModelState::default();
        let mut ctx = exec_ctx(&models);
        assert!(matches!(
            LangCommand.run(&mut ctx, "en"),
            CommandResult::Message(_)
        ));
        assert_eq!(i18n::current_lang(), Lang::En);
        assert!(matches!(
            LangCommand.run(&mut ctx, "zh"),
            CommandResult::Message(_)
        ));
        assert_eq!(i18n::current_lang(), Lang::Zh);
        i18n::set_lang(before);
    }

    #[test]
    fn rejects_unknown_args() {
        let models = ModelState::default();
        let mut ctx = exec_ctx(&models);
        assert!(matches!(
            LangCommand.run(&mut ctx, "fr"),
            CommandResult::Error(_)
        ));
    }
}
