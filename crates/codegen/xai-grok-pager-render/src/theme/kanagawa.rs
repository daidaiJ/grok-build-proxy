//! Kanagawa, ported from the sibling agent opencode's theme library
//! (`packages/tui/src/theme/assets/kanagawa.json`, dark variant).
//!
//! Donor slots → grok2 slots: primary(crystalBlue)→accent_user,
//! accent(sakuraPink)→accent_assistant, secondary(oniViolet)→accent_running,
//! error/warning/success/info→同名 accent, background(sumiInk0)→bg_base,
//! backgroundPanel(sumiInk1)→bg_light, backgroundElement(sumiInk2)→
//! bg_highlight, oldWhite→text_secondary, diffAddedBg/RemovedBg、
//! diffLineNumber、markdown* 直接采用。bg_dark 深档、bg_hover/bg_visual、
//! 边框细档、plan 金、command/path 拆分、md 六级标题梯按 TokyoNight 先例
//! 手调派生（kanagawa 暖色只有 ronin 一个，command/path 在 ronin 与
//! carpYellow 间拆分）。

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    pub const fn kanagawa() -> Self {
        const INK0: Color = rgb(31, 31, 40); // #1F1F28
        const INK0_DEEP: Color = rgb(25, 25, 32); // ink0 朝黑收 0.15 手调
        const INK1: Color = rgb(42, 42, 55); // #2A2A37
        const INK2: Color = rgb(54, 54, 70); // #363646
        const INK3: Color = rgb(84, 84, 109); // #54546D
        const FUJI_WHITE: Color = rgb(220, 215, 186); // #DCD7BA
        const OLD_WHITE: Color = rgb(200, 192, 147); // #C8C093
        const FUJI_GRAY: Color = rgb(114, 113, 105); // #727169
        const ONI_VIOLET: Color = rgb(149, 127, 184); // #957FB8
        const CRYSTAL_BLUE: Color = rgb(126, 156, 216); // #7E9CD8
        const CARP_YELLOW: Color = rgb(195, 141, 157); // #C38D9D
        const SAKURA_PINK: Color = rgb(210, 126, 153); // #D27E99
        const WAVE_AQUA: Color = rgb(118, 148, 106); // #76946A
        const RONIN_YELLOW: Color = rgb(215, 166, 87); // #D7A657
        const DRAGON_RED: Color = rgb(232, 36, 36); // #E82424
        const LOTUS_GREEN: Color = rgb(152, 187, 108); // #98BB6C
        const LINE_NO: Color = rgb(144, 144, 160); // donor diffLineNumber
        Self {
            bg_base: INK0,
            bg_light: INK1,
            bg_dark: INK0_DEEP,
            bg_highlight: INK2,
            bg_hover: rgb(62, 62, 80),
            bg_terminal: INK1,

            accent_user: CRYSTAL_BLUE,
            accent_assistant: SAKURA_PINK,
            accent_thinking: INK2,
            accent_tool: CARP_YELLOW,
            accent_system: CRYSTAL_BLUE,
            accent_error: DRAGON_RED,
            accent_success: LOTUS_GREEN,
            accent_running: ONI_VIOLET,
            accent_skill: rgb(88, 107, 83),

            text_primary: FUJI_WHITE,
            text_secondary: OLD_WHITE,

            gray_dim: INK3,
            gray: FUJI_GRAY,
            gray_bright: LINE_NO,

            command: RONIN_YELLOW,
            path: CARP_YELLOW,
            running: WAVE_AQUA,
            warning: RONIN_YELLOW,

            fuzzy_accent: CRYSTAL_BLUE,

            accent_plan: rgb(222, 180, 100),
            accent_verify: ONI_VIOLET,
            // donor diffHighlightAdded（亮黄绿），与 success 区分
            accent_remember: rgb(169, 217, 119),

            selection_border: rgb(76, 85, 114),
            hover_border: rgb(69, 69, 88),
            prompt_border: rgb(64, 64, 82),
            prompt_border_active: rgb(94, 98, 130),

            accent_model: WAVE_AQUA,

            scrollbar_bg: INK0_DEEP,
            scrollbar_fg: INK2,

            diff_delete_bg: rgb(54, 32, 32),
            diff_delete_fg: DRAGON_RED,
            diff_insert_bg: rgb(37, 46, 37),
            diff_insert_fg: LOTUS_GREEN,
            diff_equal_fg: FUJI_GRAY,
            diff_gutter_fg: LINE_NO,

            bg_visual: rgb(50, 56, 75),

            paste_bg: INK0_DEEP,
            paste_fg: OLD_WHITE,
            paste_dim: FUJI_GRAY,

            md_heading_h1: WAVE_AQUA,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: CRYSTAL_BLUE,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: RONIN_YELLOW,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: DRAGON_RED,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: LOTUS_GREEN,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: ONI_VIOLET,
            md_heading_h6_mod: Modifier::BOLD,
            md_code: LOTUS_GREEN,
            md_task_checked: WAVE_AQUA,
            md_task_unchecked: CRYSTAL_BLUE,
            md_muted: FUJI_GRAY,
            md_code_bg: INK2,
            md_text: FUJI_WHITE,
            link_fg: CRYSTAL_BLUE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeKind;

    #[test]
    fn kanagawa_signature_colors() {
        let t = Theme::kanagawa();
        assert_eq!(t.bg_base, Color::Rgb(31, 31, 40));
        assert_eq!(t.accent_user, Color::Rgb(126, 156, 216));
        assert!(t.is_dark());
        assert_eq!(ThemeKind::Kanagawa.display_name(), "kanagawa");
    }
}
