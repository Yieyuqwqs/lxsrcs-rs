//! 小工具：UA、哈希、签名、内置脚本渲染、静音 mp3。

use crate::config::Config;

/// 请求上游/对外统一的 UA。
pub fn default_ua() -> String {
    format!("Mozilla/5.0 (compatible; lxsrcs-rs/{})", env!("CARGO_PKG_VERSION"))
}

pub fn md5_hex(input: &str) -> String {
    format!("{:x}", md5::compute(input.as_bytes()))
}

pub fn sha1_hex(input: &str) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// 错误音频签名：`md5(时间戳 + 密钥)` 前 8 位。密钥为空时用配置版本号兜底。
pub fn error_sign(cfg: &Config, ts: u64) -> String {
    let secret = if cfg.data.local.sign.is_empty() { cfg.version.as_str() } else { cfg.data.local.sign.as_str() };
    let mut sign = md5_hex(&format!("{ts}{secret}"));
    sign.truncate(8);
    sign
}

/// 校验错误音频签名，有效期 `Errs.Expi` 秒（默认 120）。
pub fn error_sign_ok(cfg: &Config, sign: &str) -> bool {
    let now = crate::cache::now_secs();
    let window = cfg.errs.expi.max(60);
    for offset in [0u64, 1, 2] {
        if now < offset * window {
            continue;
        }
        let ts = now - offset * window;
        if error_sign(cfg, ts) == sign {
            return true;
        }
    }
    false
}

/// 把内置脚本里的地址与密钥占位替换掉（客户端会把这个脚本当自定义源导入）。
pub fn render_script(cfg: &Config, template: &str, addr: &str, key: &str) -> String {
    let _ = cfg;
    template
        .replace("__ADDR__", &format!("'{}'", addr.trim_end_matches('/')))
        .replace("__PASS__", &format!("'{}'", key))
}

/// 1 秒左右的静音 MPEG-1 Layer3 帧（128kbps / 44.1kHz），给播放器一个能读的兜底音源。
pub fn silence_mp3() -> Vec<u8> {
    let mut out = Vec::with_capacity(40 * 417);
    for _ in 0..40 {
        out.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x64]);
        out.extend(std::iter::repeat(0u8).take(413));
    }
    out
}

/// 秒 → 分钟:秒 文本（日志用）。
pub fn fmt_duration(secs: u64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{d}天{h}时{m}分{s}秒")
}

/// 从字符串里抠数字 ID（有些平台 ID 带前缀，如 `MUSIC_228908`）。
pub fn digits(input: &str) -> String {
    input.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// 音质档位排序：越大越好，用于回落策略与「实际音质」比较。
pub fn quality_rank(qual: &str) -> u8 {
    match qual {
        "flac24bit" | "hires" | "master" | "dolby" | "sky" => 9,
        "flac" => 8,
        "320k" => 6,
        "192k" => 4,
        "128k" => 2,
        "temp" => 0,
        _ => 1,
    }
}

/// 目标音质之前的所有档位（含自身），从高到低，用于「请求 flac 但只有 320k」的回落链。
pub fn quality_chain(qual: &str) -> Vec<&'static str> {
    match qual {
        "flac24bit" => vec!["flac24bit", "flac", "320k", "192k", "128k"],
        "flac" => vec!["flac", "320k", "192k", "128k"],
        "320k" => vec!["320k", "192k", "128k"],
        "192k" => vec!["192k", "128k"],
        "128k" => vec!["128k"],
        _ => vec!["320k", "192k", "128k"],
    }
}
