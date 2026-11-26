//! 音源注册表：平台能力声明 + 音质回落 + 统一错误。
//!
//! 每个平台只实现自己真有的方法；调不到的方法返回「不支持的平台或功能」，
//! 由路由层翻成 `code=6`，绝不假装成功（静默降级比报错更难查）。

pub mod bi;
pub mod kg;
pub mod kw;
pub mod lx;
pub mod mg;
pub mod tx;
pub mod wy;

use crate::{
    config::Config,
    itype::{Link, LyricInfo, MusicInfo, SearchPage},
    state::AppState,
    util,
};

/// 平台二字码（与 Go 版一致）
pub const PLATFORMS: [&str; 7] = ["wy", "mg", "kw", "kg", "tx", "bi", "lx"];

pub const E_DISABLED: &str = "该音乐源已被禁用";
pub const E_UNSUPPORTED: &str = "不支持的平台或功能";
pub const E_QUALITY: &str = "不支持的音质";
pub const E_ID: &str = "音乐ID校验失败";
pub const E_NO_LINK: &str = "无法获取音乐链接";

/// 平台对外声明的音质列表；`None` 表示该平台不可用（脚本里会显示为 null）。
pub fn qualities(cfg: &Config, plat: &str) -> Option<Vec<&'static str>> {
    match plat {
        "wy" if cfg.srcs.wy.enable => Some(vec!["128k", "320k", "flac", "flac24bit"]),
        "mg" if cfg.srcs.mg.enable => Some(vec!["128k", "320k", "flac", "flac24bit"]),
        "kw" if cfg.srcs.kw.enable => Some(vec!["128k", "320k", "flac"]),
        "kg" if cfg.srcs.kg.enable && !cfg.srcs.kg.node.is_empty() => Some(vec!["128k", "320k", "flac", "flac24bit"]),
        "tx" if cfg.srcs.tx.enable => Some(vec!["128k", "320k", "flac", "flac24bit"]),
        "bi" if cfg.srcs.bi.enable => Some(vec!["128k", "320k", "flac", "flac24bit"]),
        "lx" if cfg.srcs.lx.enable => Some(vec!["orig"]),
        _ => None,
    }
}

pub fn platform_enabled(cfg: &Config, plat: &str) -> bool {
    qualities(cfg, plat).is_some()
}

/// 平台 → 直链缓存时长（秒），照抄 Go 版 `C_*` 常量。
pub fn cache_ttl(plat: &str) -> u64 {
    match plat {
        "wy" => 20 * 60,
        "kw" => 60 * 60,
        "kg" => 24 * 60 * 60,
        "mg" => 24 * 60 * 60,
        "tx" => 80400,
        "bi" => 60 * 60,
        _ => 12 * 60 * 60,
    }
}

/// 校验 ID（长度/字符），与 Go 版 `Vef` 同义。
pub fn verify_id(plat: &str, id: &str) -> bool {
    match plat {
        "wy" | "kw" => id.parse::<u64>().is_ok(),
        "mg" => id.len() == 11,
        "kg" => id.len() == 32,
        "tx" => id.len() == 14,
        "bi" => !id.is_empty(),
        "lx" => !id.is_empty(),
        _ => false,
    }
}

/// 取一条直链：带音质回落，返回**实际**音质。
pub async fn resolve_url(st: &AppState, plat: &str, id: &str, qual: &str) -> Result<Link, String> {
    if examples_disabled(st, plat) {
        return Err(E_DISABLED.into());
    }
    let chain = util::quality_chain(qual);
    let mut last_err = String::new();
    for candidate in chain {
        let result = match plat {
            "wy" => wy::url(st, id, candidate).await,
            "kw" => kw::url(st, id, candidate).await,
            "kg" => kg::url(st, id, candidate).await,
            "mg" => mg::url(st, id, candidate).await,
            "tx" => tx::url(st, id, candidate).await,
            "bi" => bi::url(st, id, candidate).await,
            "lx" => lx::url(st, id, candidate).await,
            _ => return Err(format!("{E_UNSUPPORTED}: {plat}")),
        };
        match result {
            Ok(link) => {
                // 上游可能降档：如实上报实际档位
                let actual = if util::quality_rank(&link.qual) > util::quality_rank(candidate) {
                    link.qual.clone()
                } else if link.qual.is_empty() {
                    candidate.to_string()
                } else {
                    link.qual.clone()
                };
                return Ok(Link { qual: actual, ..link });
            }
            Err(err) => last_err = err,
        }
    }
    Err(if last_err.is_empty() { E_NO_LINK.to_string() } else { last_err })
}

pub async fn resolve_lyric(st: &AppState, plat: &str, id: &str) -> Result<LyricInfo, String> {
    match plat {
        "wy" => wy::lyric(st, id).await,
        "kw" => kw::lyric(st, id).await,
        "kg" => kg::lyric(st, id).await,
        "mg" => mg::lyric(st, id).await,
        "tx" => tx::lyric(st, id).await,
        _ => Err(format!("{E_UNSUPPORTED}: {plat}")),
    }
}

pub async fn resolve_pic(st: &AppState, plat: &str, id: &str, size: &str) -> Result<String, String> {
    match plat {
        "wy" => wy::pic(st, id, size).await,
        "kw" => kw::pic(st, id, size).await,
        "kg" => kg::pic(st, id, size).await,
        "mg" => mg::pic(st, id, size).await,
        "tx" => tx::pic(st, id, size).await,
        "bi" => bi::pic(st, id, size).await,
        _ => Err(format!("{E_UNSUPPORTED}: {plat}")),
    }
}

pub async fn resolve_info(st: &AppState, plat: &str, id: &str) -> Result<MusicInfo, String> {
    match plat {
        "wy" => wy::info(st, id).await,
        "kw" => kw::info(st, id).await,
        "kg" => kg::info(st, id).await,
        "mg" => mg::info(st, id).await,
        "tx" => tx::info(st, id).await,
        "bi" => bi::info(st, id).await,
        _ => Err(format!("{E_UNSUPPORTED}: {plat}")),
    }
}

pub async fn resolve_search(st: &AppState, plat: &str, keyword: &str, page: u32, size: u32) -> Result<SearchPage, String> {
    match plat {
        "wy" => wy::search(st, keyword, page, size).await,
        "kw" => kw::search(st, keyword, page, size).await,
        "kg" => kg::search(st, keyword, page, size).await,
        "mg" => mg::search(st, keyword, page, size).await,
        "tx" => tx::search(st, keyword, page, size).await,
        _ => Err(format!("{E_UNSUPPORTED}: {plat}")),
    }
}

fn examples_disabled(st: &AppState, plat: &str) -> bool {
    !platform_enabled(&st.cfg, plat)
}
