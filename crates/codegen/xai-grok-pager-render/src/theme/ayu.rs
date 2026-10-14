//! Ayu (dark), ported from the sibling agent qwen-code's theme definitions
//! (`packages/cli/src/ui/themes/ayu.ts`, the community Ayu dark palette).
//!
//! Donor slots → grok2 slots: Foreground(#aeaca6)→text_primary/md_text,
//! LightBlue(#59C2FF)→accent_user/accent_system/fuzzy_accent, AccentBlue
//! (#39BAE6)→accent_assistant/accent_running/link_fg/prompt_border_active,
//! AccentPurple(#D2A6FF)→accent_verify/accent_model, AccentCyan(#95E6CB)→
//! accent_skill/md_code, AccentGreen(#AAD94C)→success/remember/diff_insert_fg,
//! AccentYellow(#FFB454)→command/path/warning/plan（暖橙日落是本主题的辨识位）,
//! AccentRed(#F26D78)→error/diff_delete_fg, Comment(#646A71)→gray/tool/
//! md_muted, DiffAdded/Removed(#293022/#3D1215)→diff 底色直采。
//! donor 未提供的槽位（底色梯度、hover、边框细档、md 标题梯、次级文本、
//! 滚动条）按 Nord 先例在 ayu 蓝黑色系内手调派生，偏移比例在行内注明。

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    pub const fn ayu() -> Self {
        const BG: Color = rgb(11, 14, 20); // donor Background #0B0E14
        const FG: Color = rgb(174, 172, 166); // donor Foreground #AEACA6
        const LIGHT_BLUE: Color = rgb(89, 194, 255); // donor LightBlue #59C2FF
        const BLUE: Color = rgb(57, 186, 230); // donor AccentBlue #39BAE6
        const PURPLE: Color = rgb(210, 166, 255); // donor AccentPurple #D2A6FF
        const CYAN: Color = rgb(149, 230, 203); // donor AccentCyan #95E6CB
        const GREEN: Color = rgb(170, 217, 76); // donor AccentGreen #AAD94C
        const ORANGE: Color = rgb(255, 180, 84); // donor AccentYellow #FFB454
        const RED: Color = rgb(242, 109, 120); // donor AccentRed #F26D78
        const COMMENT: Color = rgb(100, 106, 113); // donor Comment #646A71
        const SLATE: Color = rgb(108, 115, 134); // COMMENT 朝冷灰提 0.2 手调

        // 底色梯度：bg_base 朝黑收 0.12 为深档；抬升档在蓝黑色系内逐级 +15%
        Self {
            bg_base: BG,
            bg_light: rgb(22, 27, 36), // BG + 抬升一档
            bg_dark: rgb(8, 10, 15),
            bg_highlight: rgb(28, 34, 48),
            bg_hover: rgb(35, 43, 59),
            bg_terminal: rgb(8, 10, 15),

            accent_user: LIGHT_BLUE,
            accent_assistant: BLUE,
            accent_thinking: rgb(22, 27, 36),
            accent_tool: COMMENT,
            accent_system: LIGHT_BLUE,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: BLUE,
            accent_skill: CYAN,

            text_primary: FG,
            text_secondary: rgb(122, 130, 142), // FG/COMMENT 之间冷灰手调

            gray_dim: rgb(74, 80, 88), // COMMENT 朝 bg 收 0.2
            gray: COMMENT,
            gray_bright: rgb(138, 145, 157), // COMMENT 朝 fg 提 0.3

            command: ORANGE,
            path: ORANGE,
            running: LIGHT_BLUE,
            warning: ORANGE,

            fuzzy_accent: LIGHT_BLUE,

            accent_plan: ORANGE,
            accent_verify: PURPLE,
            accent_remember: GREEN,

            selection_border: rgb(39, 117, 146), // BLUE 朝 bg 收 0.4
            hover_border: rgb(35, 43, 59),
            prompt_border: rgb(28, 34, 48),
            prompt_border_active: BLUE,

            accent_model: PURPLE,

            scrollbar_bg: rgb(8, 10, 15),
            scrollbar_fg: rgb(28, 34, 48),

            diff_delete_bg: rgb(61, 18, 21), // donor DiffRemoved #3D1215
            diff_delete_fg: RED,
            diff_insert_bg: rgb(41, 48, 34), // donor DiffAdded #293022
            diff_insert_fg: GREEN,
            diff_equal_fg: COMMENT,
            diff_gutter_fg: SLATE,

            bg_visual: rgb(37, 46, 64),

            paste_bg: rgb(8, 10, 15),
            paste_fg: FG,
            paste_dim: COMMENT,

            md_heading_h1: ORANGE,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: LIGHT_BLUE,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: PURPLE,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: BLUE,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: CYAN,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: rgb(122, 130, 142),
            md_heading_h6_mod: Modifier::ITALIC,
            md_code: CYAN,
            md_task_checked: GREEN,
            md_task_unchecked: COMMENT,
            md_muted: COMMENT,
            md_code_bg: rgb(22, 27, 36),
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
    fn ayu_signature_colors() {
        let t = Theme::ayu();
        assert_eq!(t.bg_base, Color::Rgb(11, 14, 20)); // #0B0E14
        assert_eq!(t.command, Color::Rgb(255, 180, 84)); // #FFB454 warm signature
        assert!(t.is_dark());
        assert_eq!(ThemeKind::Ayu.display_name(), "ayu");
    }
}
