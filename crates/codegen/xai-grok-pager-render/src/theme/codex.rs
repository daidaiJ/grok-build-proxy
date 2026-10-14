//! OpenAI Codex CLI's TUI identity palette, ported from the sibling agent's
//! real UI colors (`codex-rs/tui/src/style.rs` + `color.rs` + bundled tmTheme
//! foreground rules) — the ChatGPT-blue system, deliberately excluding the
//! 256-color accessibility variants (ada/babbage/curie/cushman/dali/davinci,
//! gold/orange `codex.accent`) per user decision.
//!
//! Donor slots → grok2 slots: UI_ACCENT(CHATGPT_BLUE_200 #63A8F8)→accent_user/
//! accent_system/fuzzy_accent/running/link_fg, CHATGPT_BLUE_100(#A4CDFB)→
//! accent_assistant/accent_model/path, warning_notice dark preferred
//! (196,167,103)→command/warning/accent_plan, secondary_text (fg/bg 60%
//! blend → #999999 on near-black)→text_secondary, user_message_bg (white 12%
//! over base → #303030)→bg_light, tmTheme dark foreground #D0D0D0→
//! text_primary, comment #808080→gray, punctuation #BCBCBC→gray_bright,
//! markup.inserted/deleted (#00AF00/#FF5F5F)→success/insert vs error/delete.
//! Donor 未提供的槽位（深/浅底梯度、hover、边框细档、md 标题梯）按 Nord 先例
//! 在「ChatGPT 蓝 + 中性灰」色系内手调派生；md 标题 modifiers 对齐 codex
//! markdown_render 的 h1 bold+underlined / h3 bold+italic。

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    pub const fn codex() -> Self {
        const BLUE_200: Color = rgb(99, 168, 248); // donor UI_ACCENT #63A8F8
        const BLUE_100: Color = rgb(164, 205, 251); // donor CHATGPT_BLUE_100 #A4CDFB
        const BLUE_MID: Color = rgb(126, 182, 245); // BLUE_200/BLUE_100 中点，色系内派生
        const BLUE_DIM: Color = rgb(74, 127, 193); // BLUE_200 朝 bg 收 0.4 手调
        const AMBER: Color = rgb(196, 167, 103); // donor warning_notice dark #C4A767
        const GREEN: Color = rgb(0, 175, 0); // donor markup.inserted #00AF00
        const RED: Color = rgb(255, 95, 95); // donor markup.deleted #FF5F5F
        const CYAN: Color = rgb(95, 215, 255); // donor tmTheme support.type #5FD7FF
        const TEXT: Color = rgb(208, 208, 208); // donor tmTheme dark foreground #D0D0D0
        const MUTED: Color = rgb(153, 153, 153); // donor secondary_text (fg/bg 60% blend)
        const LINE_NO: Color = rgb(110, 110, 110); // MUTED 朝 bg 收 0.28 手调

        Self {
            // codex 跟随终端底色：近黑中性底 + 白 12%/8% 叠底的提示块梯度
            bg_base: rgb(20, 20, 20),
            bg_light: rgb(48, 48, 48), // donor user_message_bg（白 12% 叠底）
            bg_dark: rgb(13, 13, 13),
            bg_highlight: rgb(39, 39, 39), // 白 8% 叠底
            bg_hover: rgb(46, 46, 46),
            bg_terminal: rgb(13, 13, 13),

            accent_user: BLUE_200,
            accent_assistant: BLUE_100,
            accent_thinking: rgb(39, 39, 39),
            accent_tool: rgb(140, 140, 140),
            accent_system: BLUE_200,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: BLUE_100,
            accent_skill: CYAN,

            text_primary: TEXT,
            text_secondary: MUTED,

            gray_dim: rgb(92, 92, 92),
            gray: rgb(128, 128, 128), // donor comment #808080
            gray_bright: rgb(188, 188, 188), // donor punctuation #BCBCBC

            command: AMBER,
            path: BLUE_100,
            running: BLUE_200,
            warning: AMBER,

            fuzzy_accent: BLUE_200,

            accent_plan: AMBER,
            accent_verify: BLUE_MID,
            accent_remember: GREEN,

            selection_border: BLUE_DIM,
            hover_border: rgb(58, 58, 58),
            prompt_border: rgb(51, 51, 51),
            prompt_border_active: BLUE_DIM,

            accent_model: BLUE_100,

            scrollbar_bg: rgb(13, 13, 13),
            scrollbar_fg: rgb(48, 48, 48),

            diff_delete_bg: rgb(46, 29, 29), // 基底叠 ~10% 红手调
            diff_delete_fg: RED,
            diff_insert_bg: rgb(29, 43, 29), // 基底叠 ~10% 绿手调
            diff_insert_fg: GREEN,
            diff_equal_fg: rgb(128, 128, 128),
            diff_gutter_fg: LINE_NO,

            bg_visual: rgb(42, 42, 42),

            paste_bg: rgb(13, 13, 13),
            paste_fg: TEXT,
            paste_dim: rgb(128, 128, 128),

            md_heading_h1: BLUE_200,
            // bitflags 的 `|` 非 const，const fn 里用 from_bits_truncate 组合
            md_heading_h1_mod: Modifier::from_bits_truncate(
                Modifier::BOLD.bits() | Modifier::UNDERLINED.bits(),
            ),
            md_heading_h2: BLUE_100,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: TEXT,
            md_heading_h3_mod: Modifier::from_bits_truncate(
                Modifier::BOLD.bits() | Modifier::ITALIC.bits(),
            ),
            md_heading_h4: TEXT,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: MUTED,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: MUTED,
            md_heading_h6_mod: Modifier::ITALIC,
            md_code: CYAN,
            md_task_checked: GREEN,
            md_task_unchecked: rgb(128, 128, 128),
            md_muted: rgb(128, 128, 128),
            md_code_bg: rgb(38, 38, 38),
            md_text: TEXT,
            link_fg: BLUE_200,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeKind;

    #[test]
    fn codex_signature_colors() {
        let t = Theme::codex();
        assert_eq!(t.bg_base, Color::Rgb(20, 20, 20));
        assert_eq!(t.accent_user, Color::Rgb(99, 168, 248)); // ChatGPT Blue 200
        assert_eq!(t.bg_light, Color::Rgb(48, 48, 48)); // white 12% fill
        assert!(t.is_dark());
        assert_eq!(ThemeKind::Codex.display_name(), "codex");
    }
}
