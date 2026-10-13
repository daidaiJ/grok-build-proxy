//! Gruvbox Dark, ported from the sibling agent opencode's theme library
//! (`packages/tui/src/theme/assets/gruvbox.json`, dark variant).
//!
//! Donor slots → grok2 slots: primary(blueBright)→accent_user,
//! accent(aquaBright)→accent_assistant, secondary(purpleBright)→accent_running,
//! error/warning(redBright/orangeBright)→同名 accent, background(bg0)→bg_base,
//! backgroundPanel(bg1)→bg_light, bg2→bg_highlight, diffAddedBg/RemovedBg、
//! diffLineNumber、markdown* 语义槽直接采用。bg0_h/bg0_s/fg3 为 gruvbox 官方
//! 配套深浅档（donor 未列，属同一设计语言），bg_hover/bg_visual、边框细档、
//! plan 金、md 六级标题梯按 TokyoNight 先例手调派生。

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    pub const fn gruvbox_dark() -> Self {
        const BG0: Color = rgb(40, 40, 40); // #282828
        const BG0_H: Color = rgb(29, 32, 33); // #1d2021 官方 hard dark
        const BG0_S: Color = rgb(50, 48, 47); // #32302f 官方 soft dark
        const BG1: Color = rgb(60, 56, 54); // #3c3836
        const BG2: Color = rgb(80, 73, 69); // #504945
        const FG1: Color = rgb(235, 219, 178); // #ebdbb2
        const FG3: Color = rgb(189, 174, 141); // #bdae93 官方 fg3
        const FG4: Color = rgb(168, 153, 132); // #a89984 官方 fg4
        const GRAY: Color = rgb(146, 131, 116); // #928374
        const LINE_NO: Color = rgb(168, 162, 158); // donor diffLineNumber
        const RED_B: Color = rgb(251, 73, 52); // #fb4934
        const GREEN_B: Color = rgb(184, 187, 38); // #b8bb26
        const YELLOW: Color = rgb(215, 153, 33); // #d79921
        const YELLOW_B: Color = rgb(250, 189, 47); // #fabd2f
        const BLUE: Color = rgb(69, 133, 136); // #458588
        const BLUE_B: Color = rgb(131, 165, 152); // #83a598
        const PURPLE_B: Color = rgb(211, 134, 155); // #d3869b
        const AQUA_B: Color = rgb(142, 192, 124); // #8ec07c
        const ORANGE_B: Color = rgb(254, 128, 19); // #fe8019
        Self {
            bg_base: BG0,
            bg_light: BG1,
            bg_dark: BG0_H,
            bg_highlight: BG2,
            bg_hover: rgb(88, 82, 78),
            bg_terminal: BG0_S,

            accent_user: BLUE_B,
            accent_assistant: AQUA_B,
            accent_thinking: BG2,
            accent_tool: LINE_NO,
            accent_system: BLUE_B,
            accent_error: RED_B,
            accent_success: GREEN_B,
            accent_running: PURPLE_B,
            accent_skill: rgb(111, 146, 99),

            text_primary: FG1,
            text_secondary: FG3,

            gray_dim: GRAY,
            gray: FG4,
            gray_bright: LINE_NO,

            command: YELLOW,
            path: ORANGE_B,
            running: AQUA_B,
            warning: ORANGE_B,

            fuzzy_accent: BLUE_B,

            accent_plan: YELLOW_B,
            accent_verify: PURPLE_B,
            accent_remember: AQUA_B,

            selection_border: rgb(85, 97, 75),
            hover_border: rgb(75, 70, 66),
            prompt_border: rgb(70, 66, 62),
            prompt_border_active: rgb(100, 110, 92),

            accent_model: BLUE,

            scrollbar_bg: BG0_H,
            scrollbar_fg: BG1,

            diff_delete_bg: rgb(50, 41, 41),
            diff_delete_fg: RED_B,
            diff_insert_bg: rgb(50, 48, 47),
            diff_insert_fg: GREEN_B,
            diff_equal_fg: GRAY,
            diff_gutter_fg: GRAY,

            bg_visual: rgb(60, 70, 57),

            paste_bg: BG0_H,
            paste_fg: FG3,
            paste_dim: GRAY,

            md_heading_h1: AQUA_B,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: BLUE_B,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: ORANGE_B,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: RED_B,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: GREEN_B,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: PURPLE_B,
            md_heading_h6_mod: Modifier::BOLD,
            md_code: YELLOW_B,
            md_task_checked: AQUA_B,
            md_task_unchecked: BLUE_B,
            md_muted: GRAY,
            md_code_bg: BG1,
            md_text: FG1,
            link_fg: AQUA_B,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeKind;

    #[test]
    fn gruvbox_signature_colors() {
        let t = Theme::gruvbox_dark();
        assert_eq!(t.bg_base, Color::Rgb(40, 40, 40));
        assert_eq!(t.accent_user, Color::Rgb(131, 165, 152));
        assert!(t.is_dark());
        assert_eq!(ThemeKind::GruvboxDark.display_name(), "gruvbox-dark");
        assert_eq!(ThemeKind::GruvboxDark.aliases(), &["gruvbox"]);
    }
}
