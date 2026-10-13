//! One Dark, ported from the sibling agent opencode's theme library
//! (`packages/tui/src/theme/assets/one-dark.json`, dark variant).
//!
//! Donor slots → grok2 slots: primary(blue)→accent_user, accent(cyan)→
//! accent_assistant, secondary(purple)→accent_running, error/warning/success/
//! info(orange)→同名 accent, background→bg_base, backgroundAlt→bg_dark,
//! backgroundPanel→bg_light, border→bg_highlight, diff*Bg、diffLineNumber、
//! highlightAdded/Removed、markdown* 直接采用。bg_hover/bg_visual、边框细档、
//! plan 金、md 六级标题梯按 TokyoNight 先例手调派生。

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    pub const fn one_dark() -> Self {
        const BG: Color = rgb(40, 44, 52); // #282c34
        const BG_ALT: Color = rgb(33, 37, 43); // #21252b
        const BG_PANEL: Color = rgb(53, 59, 69); // #353b45
        const BORDER: Color = rgb(57, 63, 74); // #393f4a
        const FG: Color = rgb(171, 178, 191); // #abb2bf
        const FG_SOFT: Color = rgb(139, 146, 160); // fg/fgMuted 手调中间档
        const FG_MUTED: Color = rgb(92, 99, 112); // #5c6370
        const LINE_NO: Color = rgb(147, 152, 162); // donor diffLineNumber
        const PURPLE: Color = rgb(198, 120, 221); // #c678dd
        const BLUE: Color = rgb(97, 175, 239); // #61afef
        const RED: Color = rgb(224, 108, 117); // #e06c75
        const GREEN: Color = rgb(152, 195, 121); // #98c379
        const YELLOW: Color = rgb(229, 192, 123); // #e5c07b
        const ORANGE: Color = rgb(209, 154, 102); // #d19a66
        const CYAN: Color = rgb(86, 182, 194); // #56b6c2
        Self {
            bg_base: BG,
            bg_light: BG_PANEL,
            bg_dark: BG_ALT,
            bg_highlight: BORDER,
            bg_hover: rgb(63, 69, 82),
            bg_terminal: BG_ALT,

            accent_user: BLUE,
            accent_assistant: CYAN,
            accent_thinking: BG_PANEL,
            accent_tool: rgb(72, 141, 151),
            accent_system: BLUE,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: PURPLE,
            accent_skill: rgb(65, 118, 127),

            text_primary: FG,
            text_secondary: FG_SOFT,

            gray_dim: FG_MUTED,
            gray: LINE_NO,
            gray_bright: rgb(165, 172, 183),

            command: YELLOW,
            path: ORANGE,
            running: CYAN,
            warning: YELLOW,

            fuzzy_accent: BLUE,

            accent_plan: rgb(230, 185, 90),
            accent_verify: PURPLE,
            // donor diffHighlightAdded（更亮的黄绿），与 success 区分
            accent_remember: rgb(170, 212, 130),

            selection_border: rgb(69, 97, 124),
            hover_border: rgb(64, 70, 83),
            prompt_border: rgb(60, 66, 78),
            prompt_border_active: rgb(90, 104, 132),

            accent_model: CYAN,

            scrollbar_bg: BG_ALT,
            scrollbar_fg: BG_PANEL,

            diff_delete_bg: rgb(58, 45, 47),
            diff_delete_fg: RED,
            diff_insert_bg: rgb(44, 56, 43),
            diff_insert_fg: GREEN,
            diff_equal_fg: FG_MUTED,
            diff_gutter_fg: LINE_NO,

            bg_visual: rgb(51, 70, 89),

            paste_bg: BG_ALT,
            paste_fg: FG_SOFT,
            paste_dim: FG_MUTED,

            md_heading_h1: CYAN,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: BLUE,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: ORANGE,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: RED,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: GREEN,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: PURPLE,
            md_heading_h6_mod: Modifier::BOLD,
            md_code: GREEN,
            md_task_checked: CYAN,
            md_task_unchecked: BLUE,
            md_muted: FG_MUTED,
            md_code_bg: BG_PANEL,
            md_text: FG,
            link_fg: BLUE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeKind;

    #[test]
    fn one_dark_signature_colors() {
        let t = Theme::one_dark();
        assert_eq!(t.bg_base, Color::Rgb(40, 44, 52));
        assert_eq!(t.accent_user, Color::Rgb(97, 175, 239));
        assert!(t.is_dark());
        assert_eq!(ThemeKind::OneDark.aliases(), &["onedark"]);
    }
}
