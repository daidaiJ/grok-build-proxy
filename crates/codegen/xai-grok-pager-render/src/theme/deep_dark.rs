//! Deep Dark — ported from the leaked Claude Code v2.1.88 TUI source
//! (`src/utils/theme.ts`, `darkTheme`, retrieved from the public archive
//! qw0258/claude-code-v2.1.88). Claude terracotta-orange identity; named
//! `deep-dark` per user decision.
//!
//! Donor slots → grok2 slots: claude(#D77757)→accent_assistant/brand orange,
//! briefLabelYou(#7AB4E8)→accent_user, claudeBlue_FOR_SYSTEM_SPINNER(#93A5FF)
//! →accent_running/accent_system, permission(#B1B9F9)→accent_model/path/
//! fuzzy_accent, planMode(#48968C)→accent_plan, remember(#B1B9F9)→
//! accent_remember, merged(#AF87FF)→accent_verify, background(#00CCCC)→
//! accent_skill/md_code, success(#4EBA65)/error(#FF6B80)/warning(#FFC107)→
//! 语义三件套直采, bashBorder(#FD5DB1)→command, ide(#4782C8)→link_fg,
//! diffAdded/Removed(#225C2B/#7A2936)→diff 底色, diffAddedWord/RemovedWord
//! (#38A660/#B3596B)→diff 前景, userMessageBackground(+Hover)(#373737/
//! #464646)→bg_light/bg_hover, messageActionsBackground(#2C323E)→
//! bg_highlight, memoryBackground(#374146)→bg_visual, selectionBg(#264F78)→
//! selection_border, promptBorder(#888888)→gray/inactive(#999999)→
//! text_secondary/subtle(#505050)→gray_dim/promptBorderShimmer(#A6A6A6)→
//! gray_bright。donor 无终端背景（跟随终端），bg_base 按 codex 先例取近黑
//! #141414（拍板默认），深档/边框细档/md 标题梯在品牌橙+蓝紫色系内手调派生。

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    pub const fn deep_dark() -> Self {
        const CLAUDE: Color = rgb(215, 119, 87); // donor claude #D77757
        const SHIMMER: Color = rgb(235, 159, 127); // donor claudeShimmer #EB9F7F
        const SPINNER_BLUE: Color = rgb(147, 165, 255); // donor claudeBlue_FOR_SYSTEM_SPINNER #93A5FF
        const PERM_BLUE: Color = rgb(177, 185, 249); // donor permission/suggestion/remember #B1B9F9
        const SAGE: Color = rgb(72, 150, 140); // donor planMode #48968C
        const IDE_BLUE: Color = rgb(71, 130, 200); // donor ide #4782C8
        const CYAN: Color = rgb(0, 204, 204); // donor background #00CCCC
        const VIOLET: Color = rgb(175, 135, 255); // donor merged #AF87FF
        const PINK: Color = rgb(253, 93, 177); // donor bashBorder #FD5DB1
        const GREEN: Color = rgb(78, 186, 101); // donor success #4EBA65
        const RED: Color = rgb(255, 107, 128); // donor error #FF6B80
        const AMBER: Color = rgb(255, 193, 7); // donor warning #FFC107
        const WHITE: Color = rgb(255, 255, 255); // donor text #FFFFFF
        const INACTIVE: Color = rgb(153, 153, 153); // donor inactive #999999
        const BORDER_GRAY: Color = rgb(136, 136, 136); // donor promptBorder #888888
        const SUBTLE: Color = rgb(80, 80, 80); // donor subtle #505050

        // 底色梯度：donor 假设终端近黑底，bg_base 取 #141414 拍板默认；
        // 深档朝黑收 0.35，面板档直采 donor 消息底色
        Self {
            bg_base: rgb(20, 20, 20), // 拍板默认 #141414
            bg_light: rgb(55, 55, 55), // donor userMessageBackground #373737
            bg_dark: rgb(13, 13, 13),
            bg_highlight: rgb(44, 50, 62), // donor messageActionsBackground #2C323E
            bg_hover: rgb(70, 70, 70), // donor userMessageBackgroundHover #464646
            bg_terminal: rgb(13, 13, 13),

            accent_user: rgb(122, 180, 232), // donor briefLabelYou #7AB4E8
            accent_assistant: CLAUDE,
            accent_thinking: rgb(37, 37, 37),
            accent_tool: BORDER_GRAY,
            accent_system: SPINNER_BLUE,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: SPINNER_BLUE,
            accent_skill: CYAN,

            text_primary: WHITE,
            text_secondary: INACTIVE,

            gray_dim: SUBTLE,
            gray: BORDER_GRAY,
            gray_bright: rgb(166, 166, 166), // donor promptBorderShimmer #A6A6A6

            command: PINK,
            path: PERM_BLUE,
            running: SHIMMER, // donor claudeShimmer：运转中即品牌色闪烁
            warning: AMBER,

            fuzzy_accent: PERM_BLUE,

            accent_plan: SAGE,
            accent_verify: VIOLET,
            accent_remember: PERM_BLUE, // donor remember #B1B9F9

            selection_border: rgb(38, 79, 120), // donor selectionBg #264F78
            hover_border: rgb(46, 46, 46),
            prompt_border: rgb(51, 51, 51),
            prompt_border_active: CLAUDE, // Claude Code 激活输入框即品牌橙边

            accent_model: PERM_BLUE,

            scrollbar_bg: rgb(13, 13, 13),
            scrollbar_fg: rgb(51, 51, 51),

            diff_delete_bg: rgb(122, 41, 54), // donor diffRemoved #7A2936
            diff_delete_fg: rgb(179, 89, 107), // donor diffRemovedWord #B3596B
            diff_insert_bg: rgb(34, 92, 43), // donor diffAdded #225C2B
            diff_insert_fg: rgb(56, 166, 96), // donor diffAddedWord #38A660
            diff_equal_fg: BORDER_GRAY,
            diff_gutter_fg: rgb(90, 90, 90), // SUBTLE 朝 gray 提 0.2 手调

            bg_visual: rgb(55, 65, 70), // donor memoryBackground #374146

            paste_bg: rgb(13, 13, 13),
            paste_fg: WHITE,
            paste_dim: INACTIVE,

            md_heading_h1: CLAUDE,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: PERM_BLUE,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: VIOLET,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: SPINNER_BLUE,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: CYAN,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: INACTIVE,
            md_heading_h6_mod: Modifier::ITALIC,
            md_code: CYAN,
            md_task_checked: GREEN,
            md_task_unchecked: BORDER_GRAY,
            md_muted: BORDER_GRAY,
            md_code_bg: rgb(37, 37, 37),
            md_text: WHITE,
            link_fg: IDE_BLUE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeKind;

    #[test]
    fn deep_dark_signature_colors() {
        let t = Theme::deep_dark();
        assert_eq!(t.bg_base, Color::Rgb(20, 20, 20)); // #141414
        assert_eq!(t.accent_assistant, Color::Rgb(215, 119, 87)); // #D77757 Claude orange
        assert!(t.is_dark());
        assert_eq!(ThemeKind::DeepDark.display_name(), "deep-dark");
    }
}
