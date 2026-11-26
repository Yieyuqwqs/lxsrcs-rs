//! 配置：与 Go 版 `conf.toml` 同名字段，可直接复用现有配置。
//!
//! 设计取舍：整份配置读进内存后只读（`Arc<Config>`），热更新不在本期范围；
//! 字段全部 `default`，缺项就用零值，避免旧配置把程序干崩。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "Main")]
    pub main: Main,
    #[serde(rename = "Logs")]
    pub logs: Logs,
    #[serde(rename = "Apis")]
    pub apis: Apis,
    #[serde(rename = "Auth")]
    pub auth: Auth,
    #[serde(rename = "Srcs")]
    pub srcs: Srcs,
    #[serde(rename = "Lxcs")]
    pub lxcs: Lxcs,
    #[serde(rename = "Data")]
    pub data: Data,
    #[serde(rename = "Memo")]
    pub memo: Memo,
    #[serde(rename = "Errs")]
    pub errs: Errs,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: "1.1.0".into(),
            main: Main::default(),
            logs: Logs::default(),
            apis: Apis::default(),
            auth: Auth::default(),
            srcs: Srcs::default(),
            lxcs: Lxcs::default(),
            data: Data::default(),
            memo: Memo::default(),
            errs: Errs::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Main {
    #[serde(rename = "Debug")]
    pub debug: bool,
    #[serde(rename = "Listen")]
    pub listen: String,
    #[serde(rename = "Proxy")]
    pub proxy: bool,
    #[serde(rename = "RealIP")]
    pub real_ip: String,
    #[serde(rename = "Cors")]
    pub cors: bool,
    #[serde(rename = "LogFile")]
    pub log_file: String,
    #[serde(rename = "Console")]
    pub console: bool,
}

impl Default for Main {
    fn default() -> Self {
        Self {
            debug: false,
            listen: "127.0.0.1:1101".into(),
            proxy: false,
            real_ip: String::new(),
            cors: false,
            log_file: "data/log/000000.log".into(),
            console: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Logs {
    #[serde(rename = "Enable")]
    pub enable: bool,
    #[serde(rename = "Dir")]
    pub dir: String,
    #[serde(rename = "Fmt")]
    pub fmt: String,
    #[serde(rename = "Max")]
    pub max: u64,
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            enable: true,
            dir: "data/log/".into(),
            fmt: "%Y/%m/%d".into(),
            max: 4 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Apis {
    #[serde(rename = "WebLog")]
    pub web_log: bool,
    #[serde(rename = "ApiOnly")]
    pub api_only: bool,
}

impl Default for Apis {
    fn default() -> Self {
        Self { web_log: false, api_only: true }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Auth {
    #[serde(rename = "BanList")]
    pub ban_list: BanList,
    #[serde(rename = "RateLimit")]
    pub rate_limit: RateLimit,
    #[serde(rename = "ApiPass")]
    pub api_pass: ApiPass,
    #[serde(rename = "UniPass")]
    pub uni_pass: UniPass,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct BanList {
    #[serde(rename = "Enable")]
    pub enable: bool,
    #[serde(rename = "Strict")]
    pub strict: bool,
    #[serde(rename = "White")]
    pub white: Vec<String>,
    #[serde(rename = "Black")]
    pub black: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct RateLimit {
    #[serde(rename = "Enable")]
    pub enable: bool,
    #[serde(rename = "Block")]
    pub block: u32,
    #[serde(rename = "Globle")]
    pub globle: u32,
    #[serde(rename = "Single")]
    pub single: u32,
    #[serde(rename = "BanNum")]
    pub ban_num: u32,
    #[serde(rename = "BanTim")]
    pub ban_tim: u64,
}

impl Default for RateLimit {
    fn default() -> Self {
        Self { enable: false, block: 30, globle: 60, single: 15, ban_num: 5, ban_tim: 10 }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ApiPass {
    #[serde(rename = "Enable")]
    pub enable: bool,
    #[serde(rename = "Auto")]
    pub auto: bool,
    #[serde(rename = "Value")]
    pub value: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct UniPass {
    #[serde(rename = "Enable")]
    pub enable: bool,
    #[serde(rename = "Pool")]
    pub pool: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Srcs {
    #[serde(rename = "Bn")]
    pub bn: Bn,
    #[serde(rename = "Wy")]
    pub wy: Wy,
    #[serde(rename = "Mg")]
    pub mg: Mg,
    #[serde(rename = "Kw")]
    pub kw: Kw,
    #[serde(rename = "Kg")]
    pub kg: Kg,
    #[serde(rename = "Tx")]
    pub tx: Tx,
    #[serde(rename = "Bi")]
    pub bi: Bi,
    #[serde(rename = "Lx")]
    pub lx: Lx,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Bn {
    #[serde(rename = "GDStudio")]
    pub gd_studio: bool,
    #[serde(rename = "Toubiec")]
    pub toubiec: bool,
    #[serde(rename = "Vkeys")]
    pub vkeys: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Wy {
    #[serde(rename = "Enable")]
    pub enable: bool,
    #[serde(rename = "Prefetch")]
    pub prefetch: bool,
    #[serde(rename = "FakeIP")]
    pub fake_ip: String,
    #[serde(rename = "AdFree")]
    pub ad_free: bool,
    #[serde(rename = "Group")]
    pub group: Vec<WyGroup>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct WyGroup {
    #[serde(rename = "Enable")]
    pub enable: bool,
    #[serde(rename = "Node")]
    pub node: Vec<WyNode>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct WyNode {
    #[serde(rename = "Enable")]
    pub enable: bool,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Cookie")]
    pub cookie: String,
    #[serde(rename = "Refresh")]
    pub refresh: bool,
    #[serde(rename = "DailyTask")]
    pub daily_task: bool,
    #[serde(rename = "DailyListen")]
    pub daily_listen: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Mg {
    #[serde(rename = "Enable")]
    pub enable: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Kw {
    #[serde(rename = "Enable")]
    pub enable: bool,
    #[serde(rename = "Source")]
    pub source: String,
    #[serde(rename = "FakeIP")]
    pub fake_ip: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Kg {
    #[serde(rename = "Enable")]
    pub enable: bool,
    #[serde(rename = "FakeIP")]
    pub fake_ip: String,
    #[serde(rename = "Node")]
    pub node: Vec<KgNode>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct KgNode {
    #[serde(rename = "Enable")]
    pub enable: bool,
    #[serde(rename = "Plat")]
    pub plat: u32,
    #[serde(rename = "UserId")]
    pub user_id: String,
    #[serde(rename = "Token")]
    pub token: String,
    #[serde(rename = "Mid")]
    pub mid: String,
    #[serde(rename = "Dfid")]
    pub dfid: String,
    #[serde(rename = "Device")]
    pub device: String,
    #[serde(rename = "Refresh")]
    pub refresh: bool,
    #[serde(rename = "LiteSignin")]
    pub lite_signin: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Tx {
    #[serde(rename = "Enable")]
    pub enable: bool,
    #[serde(rename = "Node")]
    pub node: Vec<TxNode>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct TxNode {
    #[serde(rename = "Enable")]
    pub enable: bool,
    #[serde(rename = "Guid")]
    pub guid: String,
    #[serde(rename = "Uuin")]
    pub uuin: String,
    #[serde(rename = "Ukey")]
    pub ukey: String,
    #[serde(rename = "Refresh")]
    pub refresh: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Bi {
    #[serde(rename = "Enable")]
    pub enable: bool,
    #[serde(rename = "CDN")]
    pub cdn: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Lx {
    #[serde(rename = "Enable")]
    pub enable: bool,
    #[serde(rename = "Node")]
    pub node: Vec<LxNode>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct LxNode {
    #[serde(rename = "Enable")]
    pub enable: bool,
    #[serde(rename = "Plat")]
    pub plat: Vec<String>,
    #[serde(rename = "Addr")]
    pub addr: String,
    #[serde(rename = "Pass")]
    pub pass: String,
    #[serde(rename = "UA")]
    pub ua: String,
    #[serde(rename = "Head")]
    pub head: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Lxcs {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Desc")]
    pub desc: String,
    #[serde(rename = "Vers")]
    pub vers: String,
    #[serde(rename = "Auth")]
    pub auth: String,
    #[serde(rename = "Home")]
    pub home: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Data {
    #[serde(rename = "Mode")]
    pub mode: u32,
    #[serde(rename = "Type")]
    pub type_: u32,
    #[serde(rename = "Rule")]
    pub rule: Rule,
    #[serde(rename = "Local")]
    pub local: Local,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Rule {
    #[serde(rename = "ExcP")]
    pub exc_p: bool,
    #[serde(rename = "Plat")]
    pub plat: Vec<String>,
    #[serde(rename = "ExcQ")]
    pub exc_q: bool,
    #[serde(rename = "Qual")]
    pub qual: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Local {
    #[serde(rename = "Path")]
    pub path: String,
    #[serde(rename = "Bind")]
    pub bind: String,
    #[serde(rename = "Expi")]
    pub expi: u64,
    #[serde(rename = "Sign")]
    pub sign: String,
    #[serde(rename = "Acip")]
    pub acip: bool,
    #[serde(rename = "Dyna")]
    pub dyna: bool,
}

impl Default for Local {
    fn default() -> Self {
        Self {
            path: "data/cache".into(),
            bind: String::new(),
            expi: 1800,
            sign: String::new(),
            acip: false,
            dyna: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Memo {
    #[serde(rename = "Shards")]
    pub shards: usize,
}

impl Default for Memo {
    fn default() -> Self {
        Self { shards: 256 }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Errs {
    #[serde(rename = "Expi")]
    pub expi: u64,
    #[serde(rename = "ErrMp3")]
    pub err_mp3: String,
}

impl Default for Errs {
    fn default() -> Self {
        Self { expi: 120, err_mp3: String::new() }
    }
}

impl Config {
    /// 读配置：文件不存在就用默认值（并打一条警告），格式错误直接报错退出。
    pub fn load(path: &str) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("解析配置失败 {path}: {e}")),
            Err(e) => {
                tracing::warn!(path, error = %e, "配置文件读取失败，使用默认配置");
                Ok(Self::default())
            }
        }
    }
}
