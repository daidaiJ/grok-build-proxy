//! Dracula, ported from the sibling agent opencode's theme library
//! (`packages/tui/src/theme/assets/dracula.json`, single dark polarity).
//!
//! Donor slots → grok2 slots: primary(purple)→accent_user, accent(cyan)→
//! accent_assistant, secondary(pink)→accent_running, error/warning/success/
//! info(orange)→同名 accent, background→bg_base, backgroundPanel→bg_dark,
//! currentLine→bg_light/bg_highlight, diffAddedBg/RemovedBg→diff 底色,
//! markdown*/syntax* 语义槽→md_*。donor 未提供的槽位按 TokyoNight 先例在
//! 主题色系内手调派生（bg_hover/bg_visual、边框细档、plan 金、skill 青绿、
//! md 六级标题梯、灰阶三档）。Dracula 只有 8 个主色，多处复用是移植原貌。

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    pub const fn dracula() -> Self {
        const BG: Color = rgb(40, 42, 54); // #282a36
        const BG_PANEL: Color = rgb(33, 34, 44); // #21222c
        const BG_DEEP: Color = rgb(25, 26, 33); // #191a21
        const CURRENT_LINE: Color = rgb(68, 71, 90); // #44475a
        const FG: Color = rgb(248, 248, 242); // #f8f8f2
        const FG_SOFT: Color = rgb(160, 168, 190); // fg/comment 手调中间档
        const COMMENT: Color = rgb(98, 114, 164); // #6272a4
        const CYAN: Color = rgb(139, 233, 253); // #8be9fd
        const GREEN: Color = rgb(80, 250, 123); // #50fa7b
        const ORANGE: Color = rgb(255, 184, 108); // #ffb86c
        const PINK: Color = rgb(255, 121, 198); // #ff79c6
        const PURPLE: Color = rgb(189, 147, 249); // #bd93f9
        const RED: Color = rgb(255, 85, 85); // #ff5555
        const YELLOW: Color = rgb(241, 250, 140); // #f1fa8c
        Self {
            bg_base: BG,
            bg_light: CURRENT_LINE,
            bg_dark: BG_PANEL,
            bg_highlight: CURRENT_LINE,
            bg_hover: rgb(72, 78, 105),
            bg_terminal: BG_DEEP,

            accent_user: PURPLE,
            accent_assistant: CYAN,
            accent_thinking: CURRENT_LINE,
            // Dracula 的 comment 即其"哑蓝"，对应 TokyoNight DARK5 的工具位
            accent_tool: COMMENT,
            accent_system: PURPLE,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: PINK,
            accent_skill: rgb(104, 166, 183),

            text_primary: FG,
            text_secondary: FG_SOFT,

            gray_dim: CURRENT_LINE,
            gray: COMMENT,
            gray_bright: rgb(152, 154, 164),

            command: YELLOW,
            path: ORANGE,
            running: CYAN,
            warning: YELLOW,

            fuzzy_accent: PURPLE,

            accent_plan: rgb(245, 196, 95),
            accent_verify: PURPLE,
            accent_remember: GREEN,

            selection_border: rgb(104, 94, 138),
            hover_border: rgb(85, 85, 110),
            prompt_border: rgb(80, 82, 105),
            prompt_border_active: rgb(120, 110, 155),

            accent_model: CYAN,

            scrollbar_bg: BG_PANEL,
            scrollbar_fg: CURRENT_LINE,

            diff_delete_bg: rgb(58, 26, 26),
            diff_delete_fg: RED,
            diff_insert_bg: rgb(26, 58, 26),
            diff_insert_fg: GREEN,
            diff_equal_fg: COMMENT,
            diff_gutter_fg: COMMENT,

            bg_visual: rgb(73, 65, 97),

            paste_bg: BG_PANEL,
            paste_fg: FG_SOFT,
            paste_dim: COMMENT,

            md_heading_h1: CYAN,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: PURPLE,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: ORANGE,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: RED,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: GREEN,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: PINK,
            md_heading_h6_mod: Modifier::BOLD,
            md_code: GREEN,
            md_task_checked: CYAN,
            md_task_unchecked: PURPLE,
            md_muted: COMMENT,
            md_code_bg: CURRENT_LINE,
            md_text: FG,
            link_fg: CYAN,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeKind;

    #[test]
    fn dracula_signature_colors() {
        let t = Theme::dracula();
        assert_eq!(t.bg_base, Color::Rgb(40, 42, 54));
        assert_eq!(t.accent_user, Color::Rgb(189, 147, 249));
        assert_eq!(t.accent_assistant, Color::Rgb(139, 233, 253));
        assert!(t.is_dark());
        assert_eq!(ThemeKind::Dracula.aliases(), &[] as &[&str]);
    }
}
