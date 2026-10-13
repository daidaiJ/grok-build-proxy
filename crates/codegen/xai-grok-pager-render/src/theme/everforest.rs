//! Everforest (dark), ported from the sibling agent opencode's theme library
//! (`packages/tui/src/theme/assets/everforest.json`, dark variant).
//!
//! Donor slots → grok2 slots: primary(step9)→accent_user, accent(#d699b6)→
//! accent_assistant, secondary(#7fbbb3)→accent_tool, error/warning/success/
//! info→同名 accent, background(step1)→bg_base, backgroundPanel(step2)→
//! bg_light, backgroundElement(step3)→bg_highlight, textMuted(step11)→gray,
//! diffAdded(#4fd6be)/Removed、diff*Bg、diffLineNumber、markdown* 直接采用。
//! bg_dark 深档、bg_hover/bg_visual、边框细档、plan 金、md 六级标题梯按
//! TokyoNight 先例手调派生。

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    pub const fn everforest() -> Self {
        const STEP1: Color = rgb(45, 53, 59); // #2d353b
        const STEP1_DEEP: Color = rgb(39, 45, 50); // step1 朝黑收 0.12 手调
        const STEP2: Color = rgb(51, 60, 67); // #333c43
        const STEP3: Color = rgb(52, 63, 68); // #343f44
        const STEP4: Color = rgb(61, 72, 77); // #3d484d
        const STEP6: Color = rgb(122, 132, 120); // #7a8478
        const STEP7: Color = rgb(133, 146, 137); // #859289
        const STEP8: Color = rgb(157, 169, 160); // #9da9a0
        const STEP9: Color = rgb(167, 192, 128); // #a7c080
        const STEP10: Color = rgb(131, 192, 146); // #83c092
        const STEP12: Color = rgb(211, 198, 170); // #d3c6aa
        const RED: Color = rgb(230, 126, 128); // #e67e80
        const ORANGE: Color = rgb(230, 152, 117); // #e69875
        const GREEN: Color = rgb(167, 192, 128); // #a7c080
        const CYAN: Color = rgb(131, 192, 146); // #83c092
        const YELLOW: Color = rgb(219, 188, 127); // #dbbc7f
        const SECONDARY: Color = rgb(127, 187, 179); // donor #7fbbb3
        const ACCENT: Color = rgb(214, 153, 182); // donor #d699b6
        const DIFF_BLUE: Color = rgb(130, 139, 184); // donor diffContext #828bb8
        const LINE_NO: Color = rgb(160, 165, 167); // donor diffLineNumber
        Self {
            bg_base: STEP1,
            bg_light: STEP2,
            bg_dark: STEP1_DEEP,
            bg_highlight: STEP3,
            bg_hover: rgb(66, 76, 80),
            bg_terminal: STEP2,

            accent_user: STEP9,
            accent_assistant: ACCENT,
            accent_thinking: STEP4,
            accent_tool: SECONDARY,
            accent_system: STEP9,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: ACCENT,
            accent_skill: rgb(97, 136, 111),

            text_primary: STEP12,
            text_secondary: STEP8,

            gray_dim: STEP6,
            gray: STEP7,
            gray_bright: LINE_NO,

            command: YELLOW,
            path: ORANGE,
            running: CYAN,
            warning: YELLOW,

            fuzzy_accent: STEP9,

            accent_plan: rgb(222, 186, 110),
            accent_verify: ACCENT,
            // donor diffHighlightAdded（亮黄绿），与 success 区分
            accent_remember: rgb(184, 219, 135),

            selection_border: rgb(81, 107, 108),
            hover_border: rgb(72, 82, 86),
            prompt_border: rgb(68, 78, 84),
            prompt_border_active: rgb(100, 120, 122),

            accent_model: STEP10,

            scrollbar_bg: STEP1_DEEP,
            scrollbar_fg: STEP3,

            diff_delete_bg: rgb(55, 34, 44),
            diff_delete_fg: RED,
            diff_insert_bg: rgb(32, 48, 59),
            diff_insert_fg: rgb(79, 214, 190),
            diff_equal_fg: STEP7,
            diff_gutter_fg: LINE_NO,

            bg_visual: rgb(69, 81, 73),

            paste_bg: STEP1_DEEP,
            paste_fg: STEP8,
            paste_dim: STEP6,

            md_heading_h1: STEP10,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: DIFF_BLUE,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: ORANGE,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: RED,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: GREEN,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: ACCENT,
            md_heading_h6_mod: Modifier::BOLD,
            md_code: GREEN,
            md_task_checked: STEP10,
            md_task_unchecked: DIFF_BLUE,
            md_muted: STEP7,
            md_code_bg: STEP3,
            md_text: STEP12,
            link_fg: STEP9,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeKind;

    #[test]
    fn everforest_signature_colors() {
        let t = Theme::everforest();
        assert_eq!(t.bg_base, Color::Rgb(45, 53, 59));
        assert_eq!(t.accent_user, Color::Rgb(167, 192, 128));
        assert!(t.is_dark());
        assert_eq!(ThemeKind::Everforest.display_name(), "everforest");
    }
}
