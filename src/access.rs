//! 请求日志：输出与 Go 版 `[Serv]` 行同构的访问日志。
//!
//! 行格式（字段位置是给 `yieyu.sh` 里那段 awk 用的，改之前先看脚本）：
//!
//! ```text
//! [Serv] 21:31:05 [Info] wy 320k [200] 5.2ms "/api/v0/url/wy/2609419459/320k"
//!   $1     $2          $3  $4   $5    $6    $7    $8
//! ```
//!
//! awk 的取法：`$2`=时间、`$6`=状态码、末尾按 `/` 切 → `p[5]`=平台、`p[6]`=ID、`p[7]`=音质。
//! 所以这两个字段位置不能动，缓存命中之类的新信息只能加在行尾之外的地方。

use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use axum::{extract::Request, middleware::Next, response::Response};
use chrono::Local;

/// 是否往控制台打（对应配置 `[Main].Console`，启动时设置一次）
static CONSOLE: AtomicBool = AtomicBool::new(true);

pub fn set_console(enabled: bool) {
    CONSOLE.store(enabled, Ordering::Relaxed);
}

fn console_enabled() -> bool {
    CONSOLE.load(Ordering::Relaxed)
}

/// 耗时格式化：µs / ms / s，与 Go 版一样保留可读精度。
pub fn fmt_duration(dur: Duration) -> String {
    let micros = dur.as_micros();
    if micros < 1000 {
        format!("{micros}µs")
    } else if dur.as_secs() < 1 {
        format!("{:.1}ms", micros as f64 / 1000.0)
    } else {
        format!("{:.2}s", dur.as_secs_f64())
    }
}

/// 从路径里抠平台/ID/音质：`/api/v0/url/{plat}/{id}/{qual}`
fn pick_fields(path: &str) -> (&str, &str, &str) {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    // api / v0 / method / plat / id / qual
    if parts.len() >= 6 && parts[0] == "api" {
        return (parts[3], parts[4], parts[5]);
    }
    if parts.len() == 5 && parts[0] == "api" {
        return (parts[3], parts[4], "-");
    }
    ("-", "-", "-")
}

/// 短 ID：日志里只显示前 6 位，避免长 hash 把行撑爆。
fn short_id(id: &str) -> String {
    if id.len() <= 6 {
        id.to_string()
    } else {
        format!("{}...", &id[..6])
    }
}

/// 访问日志中间件：挂在所有路由外层，任何请求（含 4xx/5xx）都会留痕。
pub async fn access_log(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let response = next.run(req).await;
    let status = response.status().as_u16();
    let elapsed = fmt_duration(start.elapsed());
    let time = Local::now().format("%H:%M:%S").to_string();

    if console_enabled() {
        let (plat, id, qual) = pick_fields(&path);
        if plat != "-" {
            // 解析类请求：按 awk 认得的固定列位输出
            println!(
                "[Serv] {time} [Info] {plat} {qual} [{status}] {elapsed} \"{path}\"",
                qual = if qual == "-" { "none" } else { qual }
            );
            tracing::debug!(%method, %path, status, %elapsed, plat, id = %short_id(id), "请求完成");
        } else {
            println!("[Serv] {time} [Info] - - [{status}] {elapsed} \"{path}\"");
        }
    }
    response
}

/// 启动日志：模仿 Go 版 `[ServInit]` 的口径，方便肉眼确认端口。
pub fn log_startup(listen: &str, version: &str, platform_count: usize) {
    if !console_enabled() {
        return;
    }
    println!("[ServHello] 欢迎使用 LX-Source (Rust 实现) v{version}");
    println!("[ServInit] 服务端启动, 监听地址 {listen}, 可用平台 {platform_count} 个");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_units_are_readable() {
        assert!(fmt_duration(Duration::from_micros(120)).ends_with("µs"));
        assert!(fmt_duration(Duration::from_millis(12)).ends_with("ms"));
        assert!(fmt_duration(Duration::from_millis(1500)).ends_with('s'));
    }

    #[test]
    fn picks_platform_id_and_quality() {
        assert_eq!(pick_fields("/api/v0/url/wy/2609419459/320k"), ("wy", "2609419459", "320k"));
        assert_eq!(pick_fields("/api/v0/lrc/wy/1912598037/"), ("wy", "1912598037", "-"));
        assert_eq!(pick_fields("/"), ("-", "-", "-"));
    }
}
