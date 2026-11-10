// 命令定义层：只放 clap 结构，不依赖其他模块，测试用 #[path] 直接引入。
// 顶层按「副作用等级」分组，而不是按功能多少：
//   session  只读查询
//   handoff  写临时目录（导出交接文件）
//   suggest  调外部模型、产生用量（唯一计费类别，宿主可单独收紧授权）
//   skill    写宿主技能目录（技能分发）
// 同一动词在不同组里保持同一语义。

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
    /// 为某个会话生成一条 AI 建议回复（按会话所属 agent 路由生成器）
    Suggest {
        #[command(subcommand)]
        action: SuggestAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum SuggestAction {
    /// 生成建议并输出结构化结果；会真实调用该 agent 的模型
    Generate {
        /// 会话 ID，支持 codex:<uuid>、codex://threads/<uuid>、zcode:<id>、workbuddy:<id>、opencode:<id>
        id: String,
        /// 带着上一条建议要求避开，用于「换一个」
        #[arg(long = "avoid")]
        avoid: Option<String>,
        /// 覆盖生成模型（如 gpt-6.1-sol）；不给则沿用本机 Codex 配置
        #[arg(long = "model")]
        model: Option<String>,
        /// 覆盖推理档：minimal / low / medium / high；不给则沿用本机配置
        #[arg(long = "effort")]
        effort: Option<String>,
        /// 引擎超时秒数（默认 45）；high 档本地通常要更久，按需调大
        #[arg(long = "timeout")]
        timeout: Option<u64>,
        /// 生成引擎：auto（默认，先试 sample 再回落 exec）/ sample / exec
        #[arg(long = "engine")]
        engine: Option<String>,
        /// 引擎归属：session（默认，跟随会话所属 agent）/ codex（跨引擎，用 Codex 生成）
        #[arg(long = "via")]
        via: Option<String>,
        /// 源会话转写路径；跨引擎且后端无法按 id 自行定位时手动指定
        #[arg(long = "session-path")]
        session_path: Option<String>,
    },
    /// 只组装上下文与提示词，不调用模型、不写盘，用于核对 v5 prompt
    Prompt {
        /// 会话 ID，支持 codex:<uuid>、codex://threads/<uuid>、zcode:<id>、workbuddy:<id>、opencode:<id>
        id: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum SessionAction {
    /// 列出会话元数据：状态、标题、工作区、provider、时间
    List {
        /// 只看这些 provider：copilot、copilot-cli、claude、codex、opencode、zcode、workbuddy，可重复给
        #[arg(long = "provider")]
        provider: Vec<String>,
        /// 只看这个状态：waiting、running、idle
        #[arg(long = "status")]
        status: Option<String>,
        /// 只看这个工作区路径（整串相等，忽略大小写）
        #[arg(long = "workspace")]
        workspace: Option<String>,
        /// 最多返回多少个会话，默认 80；--active-days 0 时最高 20000
        #[arg(long = "limit")]
        limit: Option<usize>,
        /// 活跃窗口天数，非 0 值夹在 1 到 30，默认 7；0 表示不限时间（全历史）
        #[arg(long = "active-days")]
        active_days: Option<u64>,
        /// 连归档会话一起列出：Codex archived_sessions 目录、OpenCode/ZCode time_archived、JSONL 归档标记、WorkBuddy deleted_at 软删
        #[arg(long = "include-archived", default_value_t = false)]
        include_archived: bool,
        /// 排除 WorkBuddy playground 会话（默认纳入）
        #[arg(long = "exclude-playground", default_value_t = false)]
        exclude_playground: bool,
        /// 绕过全历史扫描缓存：不读不写缓存，直接真实扫描
        #[arg(long = "no-cache", default_value_t = false)]
        no_cache: bool,
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
    /// 自检安装状态：逐宿主给出下一步动作与汇总，供 agent 判断要不要重装
    Status {
        /// 只看这个目录
        #[arg(long = "dir")]
        dir: Option<String>,
    },
    /// 打印内嵌的用法技能正文，可用于自行写盘或与已装副本比对
    Print,
    /// 从宿主技能目录移除 AgentWatcher 用法技能
    Remove {
        /// 只从这个目录移除
        #[arg(long = "dir")]
        dir: Option<String>,
    },
}
