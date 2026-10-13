//! Vesper, ported from the sibling agent opencode's theme library
//! (`packages/tui/src/theme/assets/vesper.json`, dark variant).
//!
//! Vesper 是刻意的极简双色盘（黑底 + 琥珀/薄荷两个高亮 + 红），donor 的
//! 多个语义槽本就映射到同两个高亮色。移植保持原貌：accent_user/system/
//! plan/command/path/warning 共用琥珀 #FFC799，accent_assistant/success/
//! running 共用薄荷 #99FFE4——文件头注明这是 donor 设计本身，不是移植缺位。
//! 中间灰阶（hover、边框细档、灰阶三档）为黑底极简系手调派生。

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    pub const fn vesper() -> Self {
        const BG: Color = rgb(16, 16, 16); // #101010
        const BG_DEEP: Color = rgb(12, 12, 12); // bg 朝黑收 0.25 手调
        const BG_LIFT: Color = rgb(28, 28, 28); // donor borderSubtle #1C1C1C
        const FG: Color = rgb(255, 255, 255); // #ffffff
        const MUTED: Color = rgb(160, 160, 160); // donor #A0A0A0
        const COMMENT: Color = rgb(139, 139, 139); // #8b8b8b
        const GRAY_DIM: Color = rgb(100, 100, 100); // comment 朝 bg 收手调
        const AMBER: Color = rgb(255, 199, 153); // #FFC799
        const AMBER_SOFT: Color = rgb(230, 180, 140); // amber 朝 fg 收手调
        const MINT: Color = rgb(153, 255, 228); // #99FFE4
        const MINT_SOFT: Color = rgb(120, 200, 180); // mint 朝 bg 收手调
        const RED: Color = rgb(255, 128, 128); // #FF8080
        Self {
            bg_base: BG,
            bg_light: BG_LIFT,
            bg_dark: BG_DEEP,
            bg_highlight: BG_LIFT,
            bg_hover: rgb(34, 34, 34),
            bg_terminal: BG_DEEP,

            accent_user: AMBER,
            accent_assistant: MINT,
            accent_thinking: rgb(40, 40, 40),
            accent_tool: MUTED,
            accent_system: AMBER,
            accent_error: RED,
            accent_success: MINT,
            accent_running: MINT,
            accent_skill: rgb(147, 117, 91),

            text_primary: FG,
            text_secondary: MUTED,

            gray_dim: GRAY_DIM,
            gray: COMMENT,
            gray_bright: MUTED,

            command: AMBER,
            path: AMBER,
            running: MINT,
            warning: AMBER,

            fuzzy_accent: AMBER,

            accent_plan: AMBER,
            accent_verify: MINT,
            accent_remember: MINT,

            selection_border: rgb(104, 88, 74),
            hover_border: rgb(52, 52, 52),
            prompt_border: rgb(48, 48, 48),
            prompt_border_active: rgb(120, 100, 84),

            accent_model: AMBER,

            scrollbar_bg: BG_DEEP,
            scrollbar_fg: BG_LIFT,

            diff_delete_bg: rgb(40, 26, 26),
            diff_delete_fg: RED,
            diff_insert_bg: rgb(13, 40, 24),
            diff_insert_fg: MINT,
            diff_equal_fg: COMMENT,
            diff_gutter_fg: MUTED,

            bg_visual: rgb(52, 43, 37),

            paste_bg: BG_DEEP,
            paste_fg: MUTED,
            paste_dim: GRAY_DIM,

            md_heading_h1: AMBER,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: MINT,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: AMBER_SOFT,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: RED,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: MINT_SOFT,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: MUTED,
            md_heading_h6_mod: Modifier::BOLD,
            md_code: MINT,
            md_task_checked: MINT,
            md_task_unchecked: AMBER,
            md_muted: COMMENT,
            md_code_bg: BG_LIFT,
            md_text: FG,
            link_fg: AMBER,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeKind;

    #[test]
    fn vesper_signature_colors() {
        let t = Theme::vesper();
        assert_eq!(t.bg_base, Color::Rgb(16, 16, 16));
        assert_eq!(t.accent_user, Color::Rgb(255, 199, 153));
        assert!(t.is_dark());
        assert_eq!(ThemeKind::Vesper.display_name(), "vesper");
    }
}
