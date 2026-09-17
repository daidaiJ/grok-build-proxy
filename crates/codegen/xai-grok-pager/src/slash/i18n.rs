//! LOCAL: TUI 界面文案中英双语（斜杠命令描述/用法/参数占位符），默认中文。
//!
//! 翻译表以英文原文为键，`tr()` 在中文模式下查表替换，英文模式原样返回；
//! 表中无对应键的字符串（含 ACP/技能等运行时文案）原样透传。
//! 语言是进程级状态：默认中文，`/lang` 切换，`GROK_LANG=en` 可在启动时改默认。

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

/// UI 显示语言。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lang {
    Zh,
    En,
}

impl Lang {
    pub fn label(self) -> &'static str {
        match self {
            Lang::Zh => "中文",
            Lang::En => "English",
        }
    }
}

// 0 = 未初始化（首次读取时按 GROK_LANG 播种，缺省中文）；1 = Zh；2 = En
static LANG: AtomicU8 = AtomicU8::new(0);

fn seed_lang() -> Lang {
    let seeded = LANG.load(Ordering::Relaxed);
    if seeded != 0 {
        return if seeded == 1 { Lang::Zh } else { Lang::En };
    }
    let lang = match std::env::var("GROK_LANG").as_deref() {
        Ok("en") | Ok("en-US") | Ok("English") => Lang::En,
        Ok("zh") | Ok("zh-CN") | Ok("中文") => Lang::Zh,
        // 测试构建默认英文：上游既有测试按英文文案断言；生产默认中文
        _ if cfg!(test) => Lang::En,
        _ => Lang::Zh,
    };
    LANG.store(match lang {
        Lang::Zh => 1,
        Lang::En => 2,
    }, Ordering::Relaxed);
    lang
}

pub fn current_lang() -> Lang {
    seed_lang()
}

pub fn set_lang(lang: Lang) {
    LANG.store(match lang {
        Lang::Zh => 1,
        Lang::En => 2,
    }, Ordering::Relaxed);
}

/// 以英文原文查中文译文；英文模式或无译文时原样返回。
pub fn tr(text: &'static str) -> &'static str {
    if current_lang() == Lang::En {
        return text;
    }
    static TABLE: OnceLock<&[( &'static str, &'static str)]> = OnceLock::new();
    let table = TABLE.get_or_init(translations);
    table
        .iter()
        .find(|(en, _)| *en == text)
        .map_or(text, |(_, zh)| zh)
}

/// LOCAL: 动态字符串（ACP 命令描述、快捷键标签等运行时文案）的查表版本。
/// 英文模式原样返回；中文模式查表，命中返回译文，未命中原样拷贝。
pub fn tr_str(text: &str) -> String {
    if current_lang() == Lang::En {
        return text.to_owned();
    }
    static TABLE: OnceLock<&[( &'static str, &'static str)]> = OnceLock::new();
    let table = TABLE.get_or_init(translations);
    table
        .iter()
        .find(|(en, _)| *en == text)
        .map(|(_, zh)| (*zh).to_owned())
        .unwrap_or_else(|| text.to_owned())
}

fn translations() -> &'static [(&'static str, &'static str)] {
    &[
        // -- 斜杠命令描述 --
        (
            "Toggle always-approve mode (skip all permission prompts)",
            "切换总是批准模式（跳过所有权限确认）",
        ),
        ("Show or hide announcements", "显示或隐藏公告"),
        (
            "Toggle auto mode (classifier approves safe tools)",
            "切换自动模式（由分类器自动批准安全工具）",
        ),
        ("Ask a side question without interrupting", "旁路提问，不打断当前任务"),
        ("Compact conversation history", "压缩对话历史"),
        ("Change the working directory for new agents", "更改新代理的工作目录"),
        (
            "Toggle compact UI (less padding, more content)",
            "切换紧凑界面（更少留白，更多内容）",
        ),
        ("Manage agent definitions", "管理代理定义"),
        ("View context usage", "查看上下文用量"),
        (
            "Copy last response to clipboard or file (/copy [N] [file])",
            "复制上一条回复到剪贴板或文件 (/copy [N] [文件])",
        ),
        ("Open the Agent Dashboard", "打开代理仪表盘"),
        ("Toggle debug overlays", "切换调试浮层"),
        ("Delete this session", "删除当前会话"),
        ("Open How-to Guides or online Build docs", "打开使用指南或在线 Build 文档"),
        ("Browse in-TUI How-to Guides", "在 TUI 内浏览使用指南"),
        ("Open docs.x.ai/build in the browser", "在浏览器中打开 docs.x.ai/build"),
        (
            "Check this session and show available fixes",
            "检查当前会话并显示可用修复",
        ),
        ("Show automatic fixes available here", "显示此处可用的自动修复"),
        (
            "Open an external editor for an empty prompt; use the command palette to preserve a draft",
            "用外部编辑器编辑空提示词；用命令面板可保留草稿",
        ),
        ("Set reasoning effort for the current model", "设置当前模型的推理强度"),
        (
            "Re-print the last collapsed block, fully expanded (minimal mode)",
            "重新完整展开上一个折叠块（极简模式）",
        ),
        ("Quit the application", "退出程序"),
        (
            "Export the current conversation to a file or clipboard",
            "将当前对话导出到文件或剪贴板",
        ),
        ("Send feedback about the current session", "反馈当前会话的问题"),
        ("Search the conversation scrollback", "搜索对话回滚缓冲"),
        ("Branch the current session into a peer agent", "将当前会话分叉为对等代理"),
        ("Hidden easter egg", "隐藏彩蛋"),
        ("Browse commands and keyboard shortcuts", "浏览命令与快捷键"),
        ("Search prompt history", "搜索提示词历史"),
        ("Return to the welcome screen", "返回欢迎界面"),
        ("Generate an image from a text description", "根据文字描述生成图片"),
        ("Open the Claude settings import modal", "打开 Claude 设置导入窗口"),
        ("Generate a video from a text description", "根据文字描述生成视频"),
        ("Jump to a turn in the conversation", "跳转到对话中的某一轮"),
        ("Log in or re-authenticate with your account", "登录或重新验证账号"),
        ("Log out and return to the login screen", "登出并返回登录界面"),
        ("Run a prompt on a recurring interval", "按周期重复执行提示词"),
        ("Show MCP server status", "显示 MCP 服务器状态"),
        ("Switch the active model", "切换当前模型"),
        (
            "Toggle multiline input mode (swap Enter and Shift+Enter)",
            "切换多行输入模式（交换 Enter 与 Shift+Enter）",
        ),
        ("Start a new session", "开始新会话"),
        ("Manage personas (create, edit, delete)", "管理人格（创建、编辑、删除）"),
        ("Enter plan mode", "进入计划模式"),
        ("View hooks", "查看钩子"),
        ("View plugins", "查看插件"),
        ("View marketplace", "查看插件市场"),
        ("View skills", "查看技能"),
        ("Open coding data, retention, and training settings", "打开数据、保留与训练设置"),
        ("List the prompts queued behind the running turn", "查看排队等待的提示词"),
        ("Summarize the session so far", "总结目前为止的会话"),
        ("Save a memory note", "保存一条记忆"),
        ("View release notes for the current version", "查看当前版本的更新日志"),
        ("Resume a previous session", "恢复之前的会话"),
        ("Rename the current session", "重命名当前会话"),
        ("Rewind to a previous turn", "回退到之前的某一轮"),
        ("Toggle the scroll-diagnostics HUD", "切换滚动诊断 HUD"),
        ("Share this session via URL", "通过 URL 分享当前会话"),
        ("Open the settings modal", "打开设置窗口"),
        ("Show session info", "显示会话信息"),
        ("List background tasks, subagents, and scheduled tasks", "列出后台任务、子代理与定时任务"),
        ("Toggle the timeline sidebar", "切换时间线侧栏"),
        ("Switch the color theme", "切换配色主题"),
        ("Toggle message timestamps on/off", "开关消息时间戳"),
        (
            "Toggle terminal mouse reporting (native click-drag copy/paste)",
            "切换终端鼠标上报（原生拖选复制粘贴）",
        ),
        (
            "View the conversation transcript in your pager ($PAGER)",
            "在系统分页器 ($PAGER) 中查看对话记录",
        ),
        ("Quick tips to get the most out of Grok Build", "善用 Grok Build 的快速技巧"),
        ("View usage", "查看用量"),
        (
            "Show tool-output compression savings",
            "查看工具输出压缩收益（正向/负向与额外 I/O）",
        ),
        ("Manage billing", "管理账单"),
        ("View the current plan", "查看当前计划"),
        (
            "Toggle vim-style scrollback keybindings (j/k, h/l, g/G, y/Y, …)",
            "切换 vim 风格回滚键位 (j/k、h/l、g/G、y/Y 等)",
        ),
        (
            "Dictation (Ctrl+Space/F8; Esc/Enter to stop)",
            "语音输入 (Ctrl+Space/F8；Esc/Enter 停止)",
        ),
        (
            "Toggle dictation (Ctrl+Space/F8; Esc/Enter to stop)",
            "切换语音输入 (Ctrl+Space/F8；Esc/Enter 停止)",
        ),
        (
            "Switch this session to minimal (scrollback-native) mode, back with /fullscreen",
            "将当前会话切换到极简（回滚原生）模式，用 /fullscreen 切回",
        ),
        (
            "Switch this session to fullscreen mode, back with /minimal",
            "将当前会话切换到全屏模式，用 /minimal 切回",
        ),
        (
            "Launch a saved workflow, list runs, or manage a run (pause, resume, stop, save)",
            "运行已保存的工作流、查看或管理运行（暂停、恢复、停止、保存）",
        ),
        ("Browse installed workflows", "浏览已安装的工作流"),
        // -- 推理强度 --
        ("No reasoning", "不推理"),
        ("Minimal reasoning", "极少量推理"),
        ("Faster, lighter reasoning", "更快、更轻量的推理"),
        ("Balanced reasoning", "均衡推理"),
        ("Heavy reasoning", "高强度推理"),
        ("Extended reasoning", "扩展推理"),
        ("Maximum reasoning", "最大强度推理"),
        // -- 用法字符串（只译自由文本，解析用关键字保留英文） --
        ("/compact compaction instructions", "/compact 压缩指令"),
        ("/effort <level>", "/effort <级别>"),
        ("/imagine <description>", "/imagine <描述>"),
        ("/imagine-video <description>", "/imagine-video <描述>"),
        ("/loop [interval] <prompt>", "/loop [间隔] <提示词>"),
        ("/model <name> [effort]", "/model <名称> [强度]"),
        ("/plan [description]", "/plan [描述]"),
        ("/remember [text]", "/remember [内容]"),
        ("/rename <title> | --auto", "/rename <标题> | --auto"),
        ("/feedback [text]", "/feedback [文字]"),
        ("/find [text]", "/find [文字]"),
        ("/theme <name>", "/theme <名称>"),
        ("/export [filename]", "/export [文件名]"),
        ("/cd [path]", "/cd [路径]"),
        (
            "Set the cumulative child-agent cap (1–1,024)",
            "设置子代理累计上限 (1–1,024)",
        ),
        // -- 参数占位符 --
        ("compaction instructions", "压缩指令"),
        ("path", "路径"),
        ("[N] [file]", "[N] [文件]"),
        ("<level>", "<级别>"),
        ("[filename]", "[文件名]"),
        ("[feedback text]", "[反馈内容]"),
        ("[text]", "[文字]"),
        ("[directive]", "[指令]"),
        ("description of the image to generate", "要生成的图片描述"),
        ("description of the video to generate", "要生成的视频描述"),
        ("[interval] <prompt>", "[间隔] <提示词>"),
        ("<model> [effort]", "<模型> [强度]"),
        ("[description]", "[描述]"),
        ("<title>", "<标题>"),
        ("[memory note text]", "[记忆内容]"),
        ("<theme>", "<主题>"),
        (
            "<name> [--agent-budget N] [--effort LEVEL] [args] | runs | pause|resume|stop|save [name]",
            "<名称> [--agent-budget N] [--effort LEVEL] [参数] | runs | pause|resume|stop|save [名称]",
        ),
        // -- 快捷键提示栏（shortcuts_bar 标签，经 tr_str 在渲染时查表） --
        ("press again to", "再按一次确认:"),
        ("New Agent", "新建代理"),
        ("accept", "接受"),
        ("accept / toggle", "接受/切换"),
        ("answer", "回答"),
        ("apply", "应用"),
        ("approve", "批准"),
        ("back", "返回"),
        ("bg", "后台"),
        ("cancel", "取消"),
        ("clear", "清空"),
        ("clear search", "清除搜索"),
        ("close", "关闭"),
        ("comment", "评论"),
        ("confirm", "确认"),
        ("copy", "复制"),
        ("copy cmd", "复制命令"),
        ("copy output", "复制输出"),
        ("copy path", "复制路径"),
        ("copy pattern", "复制模式"),
        ("copy plan", "复制计划"),
        ("copy query", "复制查询"),
        ("copy url", "复制链接"),
        ("create", "创建"),
        ("decline", "拒绝"),
        ("delete", "删除"),
        ("delete row", "删除行"),
        ("dismiss", "关闭"),
        ("drill", "深入"),
        ("edit", "编辑"),
        ("edit pattern", "编辑模式"),
        ("expand", "展开"),
        ("filename", "文件名"),
        ("filter", "过滤"),
        ("fire", "触发"),
        ("fwd", "前进"),
        ("go", "跳转"),
        ("goto", "跳转"),
        ("history", "历史"),
        ("input", "输入"),
        ("keep filter", "保留筛选"),
        ("kill", "终止"),
        ("lines", "行数"),
        ("list", "列表"),
        ("mode", "模式"),
        ("nav", "导航"),
        ("next answer", "下一答案"),
        ("next choice", "下一项"),
        ("next field", "下一字段"),
        ("next option", "下一选项"),
        ("open", "打开"),
        ("plan", "计划"),
        ("prompt", "提示词"),
        ("quit", "退出"),
        ("quit plan", "退出计划"),
        ("quote", "引用"),
        ("raw", "原文"),
        ("request changes", "请求修改"),
        ("save", "保存"),
        ("save comment", "保存评论"),
        ("search", "搜索"),
        ("select", "选择"),
        ("send", "发送"),
        ("send now", "立即发送"),
        ("send to bg", "转后台"),
        ("send+open", "发送并打开"),
        ("shortcuts", "快捷键"),
        ("submit", "提交"),
        ("switch tab", "切换标签"),
        ("toggle", "切换"),
        ("view", "查看"),
        ("wrap", "换行"),
        // -- shell 经 ACP 下发的内置命令（acp_command.rs 构造时经 tr_str 查表） --
        (
            "Compress conversation history to save context window",
            "压缩对话历史以节省上下文窗口",
        ),
        ("Flush conversation memory to disk now", "立即把会话记忆写入磁盘"),
        (
            "Run memory consolidation (merge session logs into organized topics)",
            "运行记忆整理（把会话日志合并为有条目的主题）",
        ),
        ("Browse, view, and manage your memories", "浏览、查看和管理记忆"),
        ("Show context window usage and session stats", "显示上下文窗口用量与会话统计"),
        ("Trust this project for hook execution", "信任当前项目以执行钩子"),
        ("Show hooks loaded in this session", "显示本会话加载的钩子"),
        ("Add a custom hook file or directory", "添加自定义钩子文件或目录"),
        ("Remove a custom hook file or directory path", "移除自定义钩子文件或目录路径"),
        ("Remove trust for the current project", "取消对当前项目的信任"),
        (
            "Manage plugins (list, reload, trust, add, remove)",
            "管理插件（列出、重载、信任、添加、移除）",
        ),
        (
            "Reload plugins from disk (alias for /plugins reload)",
            "从磁盘重载插件（/plugins reload 的别名）",
        ),
        (
            "Show session details (model, turns, context usage)",
            "显示会话详情（模型、轮数、上下文用量）",
        ),
        (
            "Research with bounded parallel agents, cross-check evidence, and write a cited report",
            "用有上限的并行代理做研究，交叉验证证据并写出带引用的报告",
        ),
        ("Set, manage, or check an autonomous goal", "设置、管理或检查自治目标"),
        (
            "optional context about what to preserve",
            "可选：说明要保留哪些内容",
        ),
        ("path to hook file or directory", "钩子文件或目录的路径"),
        ("list | reload | trust <path> | add <path> | remove <path>", "list | reload | trust <路径> | add <路径> | remove <路径>"),
        ("<query>", "<查询>"),
        (
            "<objective> [--budget <tokens>] | status | pause | resume | clear",
            "<目标> [--budget <token 数>] | status | pause | resume | clear",
        ),
    ]
}

#[cfg(test)]
pub(crate) mod test_sync {
    // 语言是进程级全局状态，跨模块的测试用例共用这把锁串行执行，避免互踩。
    use std::sync::Mutex;
    pub(crate) static LANG_LOCK: Mutex<()> = Mutex::new(());
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_sync::LANG_LOCK;

    #[test]
    fn tr_follows_current_lang() {
        let _guard = LANG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let before = current_lang();

        set_lang(Lang::En);
        assert_eq!(tr("Quit the application"), "Quit the application");
        assert_eq!(tr("not in table"), "not in table");

        set_lang(Lang::Zh);
        assert_eq!(tr("Quit the application"), "退出程序");
        assert_eq!(tr("not in table"), "not in table");

        set_lang(before);
    }
}
