//! LOCAL: 等待模型响应时的趣味随机文案（witty loading phrases）。
//!
//! 移植自 qwen-code（Apache-2.0，gemini-cli 系）：
//! - 英文词表：`packages/web-shell/client/constants/loadingPhrases.ts` 的 `WITTY_LOADING_PHRASES_EN`
//! - 中文词表：`packages/cli/src/i18n/locales/zh.js` 的 `WITTY_LOADING_PHRASES`（按中文语境另行创作，非逐句翻译）
//! - 轮换机制：`packages/cli/src/ui/hooks/usePhraseCycler.ts`，每 15 秒随机换一条
//!
//! Rust 侧无 React 定时器，改为在每帧渲染时按「本轮已进行秒数 / 15」的时间桶 +
//! 进程种子做确定性伪随机取词：效果等价（激活期间每 15 秒换一条），且无 RNG 依赖、
//! 渲染可重放。语言取 `slash::i18n::current_lang()`（中文默认中文词表，`/lang` 可切）。

use std::time::Duration;

use crate::slash::i18n::{current_lang, Lang};

/// 每 15 秒换一条趣味文案（对齐 qwen-code 的 `PHRASE_CHANGE_INTERVAL_MS = 15000`）。
pub(crate) const PHRASE_CHANGE_INTERVAL_SECS: u64 = 15;

/// 英文词表（qwen-code `WITTY_LOADING_PHRASES_EN` 原文迁入）。
pub(crate) const WITTY_LOADING_PHRASES_EN: &[&str] = &[
    "I'm Feeling Lucky",
    "Shipping awesomeness... ",
    "Painting the serifs back on...",
    "Consulting the digital spirits...",
    "Reticulating splines...",
    "Generating witty retort...",
    "Polishing the algorithms...",
    "Don't rush perfection (or my code)...",
    "Brewing fresh bytes...",
    "Counting electrons...",
    "Engaging cognitive processors...",
    "Checking for syntax errors in the universe...",
    "One moment, optimizing humor...",
    "Untangling neural nets...",
    "Compiling brilliance...",
    "Loading wit.exe...",
    "Preparing a witty response...",
    "Just a sec, I'm debugging reality...",
    "Crafting a response worthy of your patience...",
    "Resolving dependencies... and existential crises...",
    "Garbage collecting... be right back...",
    "Converting coffee into code...",
    "Looking for a misplaced semicolon...",
    "Pre-heating the servers...",
    "Loading the next great idea...",
    "Just a moment, I'm in the zone...",
    "Hold tight, I'm crafting a masterpiece...",
    "Warp speed engaged...",
    "Don't panic...",
    "Following the white rabbit...",
    "The truth is in here... somewhere...",
    "Loading... Do a barrel roll!",
    "Finding a suitable loading screen pun...",
    "Distracting you with this witty phrase...",
    "Almost there... probably...",
    "Hmmm... let me think...",
    "That's not a bug, it's an undocumented feature...",
    "Engage.",
    "I'll be back... with an answer.",
    "Letting the thoughts marinate...",
    "Initiating thoughtful gaze...",
    "Dividing by zero... just kidding!",
    "Buffering... because even AIs need a moment.",
    "Entangling quantum particles for a faster response...",
    "Recalibrating the humor-o-meter.",
    "Enhancing... Enhancing... Still loading.",
    "Have you tried turning it off and on again? (The loading screen, not me.)",
    "Constructing additional pylons...",
    "New line? That's Ctrl+J.",
];

/// 中文词表（qwen-code 中文 locales 的 `WITTY_LOADING_PHRASES` 原文迁入）。
pub(crate) const WITTY_LOADING_PHRASES_ZH: &[&str] = &[
    // --- 职场搬砖系列 ---
    "正在努力搬砖，请稍候...",
    "老板在身后，快加载啊！",
    "头发掉光前，一定能加载完...",
    "服务器正在深呼吸，准备放大招...",
    "正在向服务器投喂咖啡...",
    // --- 大厂黑话系列 ---
    "正在赋能全链路，寻找关键抓手...",
    "正在降本增效，优化加载路径...",
    "正在打破部门壁垒，沉淀方法论...",
    "正在拥抱变化，迭代核心价值...",
    "正在对齐颗粒度，打磨底层逻辑...",
    "大力出奇迹，正在强行加载...",
    // --- 程序员自嘲系列 ---
    "只要我不写代码，代码就没有 Bug...",
    "正在把 Bug 转化为 Feature...",
    "只要我不尴尬，Bug 就追不上我...",
    "正在试图理解去年的自己写了什么...",
    "正在猿力觉醒中，请耐心等待...",
    // --- 合作愉快系列 ---
    "正在询问产品经理：这需求是真的吗？",
    "正在给产品经理画饼，请稍等...",
    // --- 温暖治愈系列 ---
    "每一行代码，都在努力让世界变得更好一点点...",
    "每一个伟大的想法，都值得这份耐心的等待...",
    "别急，美好的事物总是需要一点时间去酝酿...",
    "愿你的代码永无 Bug，愿你的梦想终将成真...",
    "哪怕只有 0.1% 的进度，也是在向目标靠近...",
    "加载的是字节，承载的是对技术的热爱...",
];

/// 当前语言下的词表。
fn phrases() -> &'static [&'static str] {
    match current_lang() {
        Lang::Zh => WITTY_LOADING_PHRASES_ZH,
        Lang::En => WITTY_LOADING_PHRASES_EN,
    }
}

/// 进程种子：进程内恒定、跨会话不同，让两个同时运行的实例不至于总显示同一条。
fn process_seed() -> u64 {
    static SEED: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *SEED.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(0)
    })
}

/// 按本轮已进行时长取一条趣味文案：每过 [`PHRASE_CHANGE_INTERVAL_SECS`] 换一条。
///
/// `turn_elapsed` 为 `None`（时长未知）时按第 0 桶取词。确定性伪随机
/// （FNV-1a 混合进程种子与时间桶），无 RNG 依赖，同帧重复渲染结果一致。
pub(crate) fn witty_phrase(turn_elapsed: Option<Duration>) -> &'static str {
    let list = phrases();
    let bucket = turn_elapsed.map_or(0, |d| d.as_secs() / PHRASE_CHANGE_INTERVAL_SECS);
    // FNV-1a：种子与桶号混合后打散，避免相邻桶取到相邻词条
    let mut h = 0xcbf2_9ce4_8422_2325u64 ^ process_seed();
    h = (h ^ bucket).wrapping_mul(0x100_0000_01b3);
    h ^= h >> 33;
    list[(h as usize) % list.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slash::i18n::{set_lang, Lang};

    /// 两份词表都非空且无空串/重复词条（迁入 qwen-code 原文，防手误改坏）。
    #[test]
    fn phrase_lists_are_nonempty_unique_and_trimmed() {
        for list in [WITTY_LOADING_PHRASES_EN, WITTY_LOADING_PHRASES_ZH] {
            assert!(list.len() >= 20, "phrase list unexpectedly small");
            let mut seen = std::collections::HashSet::new();
            for phrase in list {
                assert!(!phrase.trim().is_empty());
                assert!(seen.insert(*phrase), "duplicate phrase: {phrase}");
            }
        }
    }

    /// 同一时刻重复调用结果一致（渲染可重放），而时间推进跨桶后可能换词。
    #[test]
    fn phrase_is_deterministic_per_bucket_and_rotates() {
        let at = |secs: u64| witty_phrase(Some(Duration::from_secs(secs)));
        let first = at(0);
        assert_eq!(first, at(0));
        assert_eq!(first, witty_phrase(None), "unknown elapsed reads as bucket 0");
        // 一个 15 秒桶内稳定，跨桶的 300 个桶至少出现多条不同文案
        let distinct: std::collections::HashSet<_> =
            (0..300).map(|b| at(b * PHRASE_CHANGE_INTERVAL_SECS)).collect();
        assert!(distinct.len() > 1, "phrase never rotates across buckets");
    }

    /// 取词索引始终落在词表范围内（防取模越界/空表 panic）。
    #[test]
    fn phrase_always_resolves_within_list() {
        let list = phrases();
        for bucket in 0..1000u64 {
            let phrase = witty_phrase(Some(Duration::from_secs(bucket * PHRASE_CHANGE_INTERVAL_SECS)));
            assert!(list.contains(&phrase));
        }
    }

    /// 中英文模式各取自对应词表（i18n 语言开关生效）。
    #[test]
    fn phrase_follows_current_lang() {
        set_lang(Lang::En);
        assert!(WITTY_LOADING_PHRASES_EN.contains(&witty_phrase(Some(Duration::from_secs(0)))));
        set_lang(Lang::Zh);
        assert!(WITTY_LOADING_PHRASES_ZH.contains(&witty_phrase(Some(Duration::from_secs(0)))));
        set_lang(Lang::En);
    }
}
