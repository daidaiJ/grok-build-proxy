use crate::app::ScreenMode;

/// What to tell a user who typed a command the current mode cannot run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remedy {
    SwitchMode {
        /// Sentence fragment, parenthesized in the refusal: `"minimal is single-session"`.
        why: &'static str,
    },
    /// Imperative clause naming what to do in this mode instead.
    /// Name arrows, `Tab`, or `Ctrl+<letter>`.
    /// `Ctrl+G` is the external editor in minimal and the tasks pane everywhere else, and a bare letter resolves only under vim mode (off by default).
    UseInstead(&'static str),
    AlreadyInMode,
}

/// Which render modes a slash command functions in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeSupport {
    Both,
    FullscreenOnly(Remedy),
    MinimalOnly(Remedy),
}

impl ModeSupport {
    pub(crate) fn supports(self, mode: ScreenMode) -> bool {
        match self {
            Self::Both => true,
            Self::FullscreenOnly(_) => !mode.is_minimal(),
            Self::MinimalOnly(_) => mode.is_minimal(),
        }
    }

    pub(crate) fn refusal(self, token: &str, mode: ScreenMode) -> Option<String> {
        if self.supports(mode) {
            return None;
        }
        let (remedy, current, switch) = match self {
            Self::Both => return None,
            Self::FullscreenOnly(remedy) => (remedy, "minimal", "/fullscreen"),
            Self::MinimalOnly(remedy) => (remedy, "fullscreen", "/minimal"),
        };
        // LOCAL: i18n — 拒绝文案模板整键成键（具名占位符，运行时插值保留）；
        // why/instead 是各命令定义的 &'static str 说明（用户可见），在消费点统一查表，
        // 因此各命令文件内的英文原文无需改动。
        use crate::slash::i18n::tr;
        Some(match remedy {
            Remedy::SwitchMode { why } => tr(
                "/{token} isn't available in {current} mode ({why}). \
                 Run {switch} to switch this session.",
            )
            .replace("{token}", token)
            .replace("{current}", current)
            .replace("{why}", tr(why))
            .replace("{switch}", switch),
            Remedy::UseInstead(instead) => {
                tr("/{token} isn't available in {current} mode: {instead}.")
                    .replace("{token}", token)
                    .replace("{current}", current)
                    .replace("{instead}", tr(instead))
            }
            Remedy::AlreadyInMode => {
                tr("You're already in {current} mode.").replace("{current}", current)
            }
        })
    }
}

#[cfg(test)]
#[path = "mode_support_tests.rs"]
mod tests;
