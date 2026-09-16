// 命令定义层：只放 clap 结构，不依赖其他模块，测试用 #[path] 直接引入。
// 命令按名词分组（session / handoff），同一动词在不同组里保持同一语义。

use clap::{Parser, Subcommand};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Json,
    Human,
}

#[derive(Debug, Parser)]
#[command(
    name = "agentwatcher-cli",
    about = "AgentWatcher 的会话查询命令行，给 AI 助手和终端使用",
    version
)]
pub struct Cli {
    /// 输出格式：json 只输出一个信封文档，human 输出人读的表格
    #[arg(long, global = true, default_value = "json")]
    pub format: OutputFormat,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// 查询被监控的会话
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// 导出接续用的源会话上下文
    Handoff {
        #[command(subcommand)]
        action: HandoffAction,
    },
    /// 管理 AgentWatcher 用法技能在各 AI 宿主的安装
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum SessionAction {
    /// 列出会话元数据：状态、标题、工作区、provider、时间
    List {
        /// 只看这些 provider：copilot、copilot-cli、claude、codex、opencode、zcode，可重复给
        #[arg(long = "provider")]
        provider: Vec<String>,
        /// 只看这个状态：waiting、running、idle
        #[arg(long = "status")]
        status: Option<String>,
        /// 只看这个工作区路径（整串相等，忽略大小写）
        #[arg(long = "workspace")]
        workspace: Option<String>,
        /// 最多返回多少个会话，默认 80
        #[arg(long = "limit")]
        limit: Option<usize>,
        /// 活跃窗口天数，夹在 1 到 30，默认 7
        #[arg(long = "active-days")]
        active_days: Option<u64>,
    },
    /// 查看单个会话的详情，默认只到悬浮预览级，--content 才带正文内容
    Show {
        /// 会话 ID，形如 opencode:abc123，来自 session list
        id: String,
        /// 请求正文内容：OpenCode/Zcode 给导出文件路径，其余给原始会话文件路径
        #[arg(long = "content", default_value_t = false)]
        content: bool,
    },
    /// 查看单个会话的用量明细：token、轮次、工具调用、模型
    Usage {
        /// 会话 ID，形如 opencode:abc123
        id: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum HandoffAction {
    /// 把源会话上下文导出成 Markdown，落盘位置与 GUI 接续面板一致
    Export {
        /// 会话 ID，形如 zcode:abc123
        id: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum SkillAction {
    /// 把 AgentWatcher 用法技能装进 AI 宿主技能目录，重复执行覆盖升级
    Install {
        /// 只装进这个目录（默认探测全部已安装宿主）
        #[arg(long = "dir")]
        dir: Option<String>,
    },
    /// 查看各宿主技能目录的安装状态
    List {
        /// 只看这个目录
        #[arg(long = "dir")]
        dir: Option<String>,
    },
    /// 从宿主技能目录移除 AgentWatcher 用法技能
    Remove {
        /// 只从这个目录移除
        #[arg(long = "dir")]
        dir: Option<String>,
    },
}
