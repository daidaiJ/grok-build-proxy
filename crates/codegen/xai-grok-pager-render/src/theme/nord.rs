//! Nord, ported from the sibling agent opencode's theme library
//! (`packages/tui/src/theme/assets/nord.json`, dark variant).
//!
//! Donor slots → grok2 slots: primary(nord8)→accent_user, accent(nord7)→
//! accent_assistant, secondary(nord9)→accent_running, error/warning/success/
//! info→同名 accent, background(nord0)→bg_base, backgroundPanel(nord1)→
//! bg_light, backgroundElement(nord2)→bg_highlight, textMuted(#8B95A7)→gray,
//! diffAddedBg/RemovedBg(#3B4252)、diffLineNumber、markdown* 直接采用。
//! donor 未提供的槽位（bg_dark 深档、bg_hover/bg_visual、边框细档、plan 金、
//! md 六级标题梯）按 TokyoNight 先例在 nord 色系内手调派生。

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    pub const fn nord() -> Self {
        const NORD0: Color = rgb(46, 52, 64); // #2E3440
        const NORD0_DEEP: Color = rgb(38, 43, 54); // nord0 朝黑收 0.12 手调
        const NORD1: Color = rgb(59, 66, 82); // #3B4252
        const NORD2: Color = rgb(67, 76, 94); // #434C5E
        const NORD4: Color = rgb(216, 222, 233); // #D8DEE9
        const NORD6: Color = rgb(236, 239, 244); // #ECEFF4
        const NORD7: Color = rgb(143, 188, 187); // #8FBCBB
        const NORD8: Color = rgb(136, 192, 208); // #88C0D0
        const NORD9: Color = rgb(129, 161, 193); // #81A1C1
        const NORD11: Color = rgb(191, 97, 106); // #BF616A
        const NORD12: Color = rgb(208, 135, 112); // #D08770
        const NORD13: Color = rgb(235, 203, 139); // #EBCB8B
        const NORD14: Color = rgb(163, 190, 140); // #A3BE8C
        const NORD15: Color = rgb(180, 142, 173); // #B48EAD
        const MUTED: Color = rgb(139, 149, 167); // donor textMuted #8B95A7
        const MUTED_DIM: Color = rgb(110, 118, 133); // MUTED 朝 bg 收 0.2 手调
        const LINE_NO: Color = rgb(169, 174, 182); // donor diffLineNumber
        Self {
            bg_base: NORD0,
            bg_light: NORD1,
            bg_dark: NORD0_DEEP,
            bg_highlight: NORD2,
            bg_hover: rgb(71, 81, 98),
            bg_terminal: NORD0_DEEP,

            accent_user: NORD8,
            accent_assistant: NORD7,
            accent_thinking: NORD1,
            accent_tool: MUTED,
            accent_system: NORD8,
            accent_error: NORD11,
            accent_success: NORD14,
            accent_running: NORD9,
            accent_skill: rgb(114, 147, 149),

            text_primary: NORD6,
            text_secondary: NORD4,

            gray_dim: MUTED_DIM,
            gray: MUTED,
            gray_bright: LINE_NO,

            command: NORD13,
            path: NORD12,
            running: NORD8,
            warning: NORD13,

            fuzzy_accent: NORD8,

            accent_plan: rgb(230, 192, 118),
            accent_verify: NORD15,
            accent_remember: NORD14,

            selection_border: rgb(89, 106, 129),
            hover_border: rgb(78, 86, 103),
            prompt_border: rgb(72, 80, 97),
            prompt_border_active: rgb(100, 116, 141),

            accent_model: NORD7,

            scrollbar_bg: NORD0_DEEP,
            scrollbar_fg: NORD2,

            // donor 对 diffAddedBg/RemovedBg 均给 nord1（同底异 fg），移植原貌
            diff_delete_bg: NORD1,
            diff_delete_fg: NORD11,
            diff_insert_bg: NORD1,
            diff_insert_fg: NORD14,
            diff_equal_fg: MUTED,
            diff_gutter_fg: LINE_NO,

            bg_visual: rgb(66, 83, 96),

            paste_bg: NORD0_DEEP,
            paste_fg: NORD4,
            paste_dim: MUTED,

            md_heading_h1: NORD7,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: NORD9,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: NORD13,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: NORD11,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: NORD14,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: NORD15,
            md_heading_h6_mod: Modifier::BOLD,
            md_code: NORD14,
            md_task_checked: NORD8,
            md_task_unchecked: NORD9,
            md_muted: MUTED,
            md_code_bg: NORD2,
            md_text: NORD4,
            link_fg: NORD9,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeKind;

    #[test]
    fn nord_signature_colors() {
        let t = Theme::nord();
        assert_eq!(t.bg_base, Color::Rgb(46, 52, 64));
        assert_eq!(t.accent_user, Color::Rgb(136, 192, 208));
        assert!(t.is_dark());
        assert_eq!(ThemeKind::Nord.display_name(), "nord");
    }
}
