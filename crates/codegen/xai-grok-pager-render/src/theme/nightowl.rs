//! Night Owl (Sarah Drasner), ported from the sibling agent opencode's theme
//! library (`packages/tui/src/theme/assets/nightowl.json`, dark variant).
//!
//! Donor slots → grok2 slots: primary(blue)→accent_user, accent(purple)→
//! accent_assistant, secondary(cyan)→accent_running/semantic running,
//! error/warning/success/info→同名 accent, background→bg_base,
//! backgroundPanel→bg_light, border/borderSubtle(muted)→gray 系,
//! diffAddedBg/RemovedBg、diffLineNumber、markdown* 直接采用。bg_dark 深档、
//! bg_hover/bg_visual、边框细档、plan 金、md 六级标题梯按 TokyoNight 先例
//! 手调派生。

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    pub const fn night_owl() -> Self {
        const BG: Color = rgb(1, 22, 39); // #011627
        const BG_DEEP: Color = rgb(1, 17, 31); // bg 朝黑收 0.2 手调
        const PANEL: Color = rgb(11, 37, 58); // #0b253a
        const FG: Color = rgb(214, 222, 235); // #d6deeb
        const BLUE: Color = rgb(130, 170, 255); // #82AAFF
        const CYAN: Color = rgb(127, 219, 202); // #7fdbca
        const GREEN: Color = rgb(197, 228, 120); // #c5e478
        const YELLOW: Color = rgb(236, 196, 141); // #ecc48d
        const ORANGE: Color = rgb(247, 140, 108); // #F78C6C
        const RED: Color = rgb(239, 83, 80); // #EF5350
        const PINK: Color = rgb(255, 88, 116); // #ff5874
        const PURPLE: Color = rgb(199, 146, 234); // #c792ea
        const MUTED: Color = rgb(95, 126, 151); // #5f7e97
        const GRAY: Color = rgb(99, 119, 119); // #637777
        const LIGHT_GRAY: Color = rgb(137, 164, 187); // #89a4bb
        const LINE_NO: Color = rgb(119, 145, 166); // donor diffLineNumber
        Self {
            bg_base: BG,
            bg_light: PANEL,
            bg_dark: BG_DEEP,
            bg_highlight: rgb(16, 46, 68),
            bg_hover: rgb(22, 52, 76),
            bg_terminal: PANEL,

            accent_user: BLUE,
            accent_assistant: PURPLE,
            accent_thinking: PANEL,
            accent_tool: LIGHT_GRAY,
            accent_system: BLUE,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: PURPLE,
            accent_skill: rgb(83, 150, 145),

            text_primary: FG,
            text_secondary: LIGHT_GRAY,

            gray_dim: GRAY,
            gray: MUTED,
            gray_bright: LIGHT_GRAY,

            command: YELLOW,
            path: ORANGE,
            running: CYAN,
            warning: YELLOW,

            fuzzy_accent: BLUE,

            accent_plan: rgb(238, 190, 110),
            accent_verify: PURPLE,
            accent_remember: GREEN,

            selection_border: rgb(47, 77, 117),
            hover_border: rgb(26, 52, 74),
            prompt_border: rgb(20, 45, 68),
            prompt_border_active: rgb(60, 90, 130),

            accent_model: CYAN,

            scrollbar_bg: BG_DEEP,
            scrollbar_fg: PANEL,

            diff_delete_bg: rgb(45, 27, 27),
            diff_delete_fg: RED,
            diff_insert_bg: rgb(10, 46, 26),
            diff_insert_fg: GREEN,
            diff_equal_fg: MUTED,
            diff_gutter_fg: LINE_NO,

            bg_visual: rgb(27, 52, 82),

            paste_bg: BG_DEEP,
            paste_fg: LIGHT_GRAY,
            paste_dim: GRAY,

            md_heading_h1: CYAN,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: BLUE,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: ORANGE,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: PINK,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: GREEN,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: PURPLE,
            md_heading_h6_mod: Modifier::BOLD,
            md_code: CYAN,
            md_task_checked: CYAN,
            md_task_unchecked: BLUE,
            md_muted: MUTED,
            md_code_bg: PANEL,
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
    fn night_owl_signature_colors() {
        let t = Theme::night_owl();
        assert_eq!(t.bg_base, Color::Rgb(1, 22, 39));
        assert_eq!(t.accent_user, Color::Rgb(130, 170, 255));
        assert!(t.is_dark());
        assert_eq!(ThemeKind::NightOwl.aliases(), &["night-owl"]);
    }
}
