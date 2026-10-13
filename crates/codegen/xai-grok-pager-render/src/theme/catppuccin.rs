//! Catppuccin Mocha (dark) and Latte (light), ported from the sibling agent
//! opencode's theme library (`packages/tui/src/theme/assets/catppuccin.json`).
//!
//! Donor slots → grok2 slots: primary→accent_user, accent→accent_assistant,
//! secondary→accent_running/verify, error/warning/success/info→同名 accent,
//! background→bg_base, backgroundPanel→bg_light, backgroundElement→bg_dark,
//! border/borderActive/borderSubtle→边框系, diff*Bg/Added/Removed→diff 系,
//! markdownCode/Link/Text→md_*。donor 未提供的槽位（bg_hover/bg_visual、
//! 边框细档、plan 金色、skill 青绿、md 六级标题梯）按 TokyoNight 先例在
//! 该主题色系内手调派生，注释逐条标注。

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

/// Catppuccin Mocha palette (opencode `dark*` defs) and Latte palette
/// (opencode `light*` defs) — one donor file carries both polarities.
mod mocha {
    use super::*;
    pub const BASE: Color = rgb(30, 30, 46); // #1e1e2e
    pub const MANTLE: Color = rgb(24, 24, 37); // #181825
    pub const CRUST: Color = rgb(17, 17, 27); // #11111b
    pub const SURFACE0: Color = rgb(49, 50, 68); // #313244
    pub const SURFACE2: Color = rgb(88, 91, 112); // #585b70
    pub const OVERLAY0: Color = rgb(108, 112, 134); // #6c7086
    pub const OVERLAY1: Color = rgb(127, 132, 156); // #7f849c
    pub const OVERLAY2: Color = rgb(147, 153, 178); // #9399b2
    pub const SUBTEXT1: Color = rgb(186, 194, 222); // #bac2de
    pub const TEXT: Color = rgb(205, 214, 244); // #cdd6f4
    pub const BLUE: Color = rgb(137, 180, 250); // #89b4fa
    pub const SAPPHIRE: Color = rgb(116, 199, 236); // #74c7ec
    pub const SKY: Color = rgb(137, 220, 235); // #89dceb
    pub const TEAL: Color = rgb(148, 226, 213); // #94e2d5
    pub const GREEN: Color = rgb(166, 227, 161); // #a6e3a1
    pub const YELLOW: Color = rgb(249, 226, 175); // #f9e2af
    pub const PEACH: Color = rgb(250, 179, 135); // #fab387
    pub const ROSEWATER: Color = rgb(245, 224, 220); // #f5e0dc
    pub const RED: Color = rgb(243, 139, 168); // #f38ba8
    pub const MAUVE: Color = rgb(203, 166, 247); // #cba6f7
    pub const PINK: Color = rgb(245, 194, 231); // #f5c2e7
}

mod latte {
    use super::*;
    pub const BASE: Color = rgb(239, 241, 245); // #eff1f5
    pub const MANTLE: Color = rgb(230, 233, 239); // #e6e9ef
    pub const CRUST: Color = rgb(220, 224, 232); // #dce0e8
    pub const SURFACE0: Color = rgb(204, 208, 218); // #ccd0da
    pub const OVERLAY0: Color = rgb(156, 160, 176); // #9ca0b0
    pub const OVERLAY1: Color = rgb(140, 143, 161); // #8c8fa1
    pub const OVERLAY2: Color = rgb(124, 127, 147); // #7c7f93
    pub const SUBTEXT1: Color = rgb(92, 95, 119); // #5c5f77
    pub const TEXT: Color = rgb(76, 79, 105); // #4c4f69
    pub const BLUE: Color = rgb(30, 102, 245); // #1e66f5
    pub const SAPPHIRE: Color = rgb(32, 159, 181); // #209fb5
    pub const SKY: Color = rgb(4, 165, 229); // #04a5e5
    pub const TEAL: Color = rgb(23, 146, 153); // #179299
    pub const GREEN: Color = rgb(64, 160, 43); // #40a02b
    pub const YELLOW: Color = rgb(223, 142, 29); // #df8e1d
    pub const PEACH: Color = rgb(254, 100, 11); // #fe640b
    pub const ROSEWATER: Color = rgb(220, 138, 120); // #dc8a78
    pub const RED: Color = rgb(210, 15, 57); // #d20f39
    pub const MAUVE: Color = rgb(136, 57, 239); // #8839ef
    pub const PINK: Color = rgb(234, 118, 203); // #ea76cb
}

impl Theme {
    /// Catppuccin Mocha (dark). Donor opencode `catppuccin.json` dark variant.
    pub const fn catppuccin_mocha() -> Self {
        use mocha::*;
        Self {
            bg_base: BASE,
            bg_light: SURFACE0,
            bg_dark: MANTLE,
            bg_highlight: SURFACE0,
            // bg_highlight 混向 bg_visual 的中点（TokyoNight bg_hover 先例）
            bg_hover: rgb(56, 58, 80),
            bg_terminal: CRUST,

            accent_user: BLUE,
            accent_assistant: PINK,
            accent_thinking: SURFACE2,
            accent_tool: TEAL,
            accent_system: BLUE,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: MAUVE,
            // TokyoNight 先例：skill 用低饱和青绿（TEAL 朝 BASE 收 0.42）
            accent_skill: rgb(110, 176, 171),

            text_primary: TEXT,
            text_secondary: SUBTEXT1,

            gray_dim: OVERLAY0,
            gray: OVERLAY1,
            gray_bright: OVERLAY2,

            command: YELLOW,
            path: PEACH,
            running: SKY,
            warning: YELLOW,

            fuzzy_accent: BLUE,

            // plan 金色：主题系内手调（YELLOW 过淡，向 PEACH 收饱和）
            accent_plan: rgb(231, 186, 108),
            accent_verify: MAUVE,
            accent_remember: GREEN,

            // selection: SURFACE1 混 BLUE 0.25；prompt/hover 为中间细档
            selection_border: rgb(86, 98, 130),
            hover_border: rgb(64, 66, 90),
            prompt_border: rgb(58, 60, 80),
            prompt_border_active: rgb(92, 96, 138),

            accent_model: SAPPHIRE,

            scrollbar_bg: MANTLE,
            scrollbar_fg: SURFACE0,

            diff_delete_bg: rgb(60, 42, 50),
            diff_delete_fg: RED,
            diff_insert_bg: rgb(36, 49, 43),
            diff_insert_fg: GREEN,
            diff_equal_fg: OVERLAY0,
            diff_gutter_fg: OVERLAY0,

            // visual: BASE 混 BLUE 0.22 的蓝色选区带
            bg_visual: rgb(54, 58, 86),

            paste_bg: MANTLE,
            paste_fg: SUBTEXT1,
            paste_dim: OVERLAY0,

            md_heading_h1: ROSEWATER,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: BLUE,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: PEACH,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: RED,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: GREEN,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: MAUVE,
            md_heading_h6_mod: Modifier::BOLD,
            md_code: GREEN,
            md_task_checked: TEAL,
            md_task_unchecked: BLUE,
            md_muted: OVERLAY1,
            md_code_bg: SURFACE0,
            md_text: TEXT,
            link_fg: BLUE,
        }
    }

    /// Catppuccin Latte (light). Donor opencode `catppuccin.json` light variant.
    pub const fn catppuccin_latte() -> Self {
        use latte::*;
        Self {
            bg_base: BASE,
            bg_light: MANTLE,
            bg_dark: CRUST,
            bg_highlight: SURFACE0,
            bg_hover: rgb(196, 199, 213),
            bg_terminal: CRUST,

            accent_user: BLUE,
            accent_assistant: PINK,
            accent_thinking: OVERLAY0,
            accent_tool: TEAL,
            accent_system: BLUE,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: MAUVE,
            accent_skill: rgb(30, 125, 130),

            text_primary: TEXT,
            text_secondary: SUBTEXT1,

            // 浅色系灰阶反向：越亮越"暗淡"，越深越"醒目"
            gray_dim: OVERLAY0,
            gray: OVERLAY1,
            gray_bright: OVERLAY2,

            command: YELLOW,
            path: PEACH,
            running: SKY,
            warning: YELLOW,

            fuzzy_accent: BLUE,

            accent_plan: YELLOW,
            accent_verify: MAUVE,
            accent_remember: GREEN,

            selection_border: rgb(141, 165, 216),
            hover_border: rgb(180, 184, 200),
            prompt_border: rgb(196, 200, 212),
            prompt_border_active: rgb(120, 148, 222),

            accent_model: SAPPHIRE,

            scrollbar_bg: CRUST,
            scrollbar_fg: SURFACE0,

            diff_delete_bg: rgb(246, 223, 226),
            diff_delete_fg: RED,
            diff_insert_bg: rgb(214, 240, 217),
            diff_insert_fg: GREEN,
            diff_equal_fg: OVERLAY1,
            diff_gutter_fg: OVERLAY0,

            bg_visual: rgb(201, 216, 245),

            paste_bg: CRUST,
            paste_fg: SUBTEXT1,
            paste_dim: OVERLAY0,

            md_heading_h1: ROSEWATER,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: BLUE,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: PEACH,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: RED,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: GREEN,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: MAUVE,
            md_heading_h6_mod: Modifier::BOLD,
            md_code: GREEN,
            md_task_checked: TEAL,
            md_task_unchecked: BLUE,
            md_muted: OVERLAY1,
            md_code_bg: SURFACE0,
            md_text: TEXT,
            link_fg: BLUE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeKind;

    #[test]
    fn mocha_signature_colors() {
        let t = Theme::catppuccin_mocha();
        assert_eq!(t.bg_base, Color::Rgb(30, 30, 46));
        assert_eq!(t.accent_user, Color::Rgb(137, 180, 250));
        assert_eq!(t.md_heading_h1, Color::Rgb(245, 224, 220));
        assert!(t.is_dark());
        assert!(ThemeKind::CatppuccinMocha.requires_truecolor());
    }

    #[test]
    fn latte_is_light() {
        let t = Theme::catppuccin_latte();
        assert_eq!(t.bg_base, Color::Rgb(239, 241, 245));
        assert!(!t.is_dark());
        // Latte 滚动条：浅底浅轨，thumb 必须比 track 深（Σ 更小）
        let sum = |c: Color| match c {
            Color::Rgb(r, g, b) => r as i32 + g as i32 + b as i32,
            other => panic!("rgb expected, got {other:?}"),
        };
        assert!(sum(t.scrollbar_fg) <= sum(t.scrollbar_bg) - 30);
    }
}
