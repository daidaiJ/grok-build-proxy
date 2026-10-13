//! Solarized Dark (Ethan Schoonover), ported from the sibling agent opencode's
//! theme library (`packages/tui/src/theme/assets/solarized.json`, dark variant).
//!
//! Donor slots → grok2 slots: primary(blue)→accent_user, accent(cyan)→
//! accent_assistant, secondary(violet)→accent_verify, error/warning/success/
//! info(orange)→同名 accent, background(base03)→bg_base, backgroundPanel/
//! border(base02)→bg_light/bg_highlight, textMuted(base01)→gray 系,
//! diff 底色（donor 双方均 base02，同底异 fg）、diffLineNumber、markdown*
//! 直接采用。Solarized 刻意低饱和：text_primary 用 base0 是原设计
//! （Sunshine 与 body 文本的经典对比），非移植缺位。bg_dark 深档、
//! bg_hover/bg_visual、边框细档、md 六级标题梯按 TokyoNight 先例手调派生。

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    pub const fn solarized_dark() -> Self {
        const BASE03: Color = rgb(0, 43, 54); // #002b36
        const BASE03_DEEP: Color = rgb(2, 36, 45); // base03 朝黑收 0.12 手调
        const BASE02: Color = rgb(7, 54, 66); // #073642
        const BASE01: Color = rgb(88, 110, 117); // #586e75
        const BASE00: Color = rgb(101, 123, 131); // #657b83
        const BASE0: Color = rgb(131, 148, 150); // #839496
        const BASE1: Color = rgb(147, 161, 161); // #93a1a1
        const YELLOW: Color = rgb(181, 137, 0); // #b58900
        const ORANGE: Color = rgb(203, 75, 22); // #cb4b16
        const RED: Color = rgb(220, 50, 47); // #dc322f
        const MAGENTA: Color = rgb(211, 54, 130); // #d33682
        const VIOLET: Color = rgb(108, 113, 196); // #6c71c4
        const BLUE: Color = rgb(38, 139, 210); // #268bd2
        const CYAN: Color = rgb(42, 161, 152); // #2aa198
        const GREEN: Color = rgb(133, 153, 0); // #859900
        const LINE_NO: Color = rgb(139, 155, 159); // donor diffLineNumber
        Self {
            bg_base: BASE03,
            bg_light: BASE02,
            bg_dark: BASE03_DEEP,
            bg_highlight: BASE02,
            bg_hover: rgb(14, 62, 74),
            bg_terminal: BASE02,

            accent_user: BLUE,
            accent_assistant: CYAN,
            accent_thinking: BASE02,
            accent_tool: BASE01,
            accent_system: BLUE,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: MAGENTA,
            accent_skill: rgb(27, 120, 118),

            text_primary: BASE0,
            text_secondary: BASE1,

            gray_dim: BASE01,
            gray: BASE00,
            gray_bright: LINE_NO,

            command: YELLOW,
            path: ORANGE,
            running: CYAN,
            warning: YELLOW,

            fuzzy_accent: BLUE,

            accent_plan: YELLOW,
            accent_verify: VIOLET,
            accent_remember: GREEN,

            selection_border: rgb(16, 80, 109),
            hover_border: rgb(13, 60, 72),
            prompt_border: rgb(10, 50, 61),
            prompt_border_active: rgb(24, 86, 116),

            accent_model: CYAN,

            scrollbar_bg: BASE03_DEEP,
            scrollbar_fg: BASE02,

            // donor 对 diffAddedBg/RemovedBg 均给 base02（同底异 fg），移植原貌
            diff_delete_bg: BASE02,
            diff_delete_fg: RED,
            diff_insert_bg: BASE02,
            diff_insert_fg: GREEN,
            diff_equal_fg: BASE01,
            diff_gutter_fg: LINE_NO,

            bg_visual: rgb(8, 62, 85),

            paste_bg: BASE03_DEEP,
            paste_fg: BASE1,
            paste_dim: BASE01,

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
            md_heading_h6: MAGENTA,
            md_heading_h6_mod: Modifier::BOLD,
            md_code: GREEN,
            md_task_checked: CYAN,
            md_task_unchecked: BLUE,
            md_muted: BASE01,
            md_code_bg: BASE02,
            md_text: BASE0,
            link_fg: CYAN,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeKind;

    #[test]
    fn solarized_signature_colors() {
        let t = Theme::solarized_dark();
        assert_eq!(t.bg_base, Color::Rgb(0, 43, 54));
        assert_eq!(t.accent_user, Color::Rgb(38, 139, 210));
        assert!(t.is_dark());
        assert_eq!(ThemeKind::SolarizedDark.aliases(), &["solarized"]);
    }
}
