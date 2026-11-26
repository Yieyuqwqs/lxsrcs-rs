//! 鉴权三件套：接口密钥、IP 黑白名单、速率限制。
//!
//! 与 Go 版一致：密钥可放请求头 `X-Request-Key` / `X-LxM-Auth`，也可放查询串 `?key=`；
//! 脚本走的是请求头（`X-LxM-Auth`），浏览器直连走的是 `?key=`。

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

use axum::http::HeaderMap;

use crate::config::Config;

/// 从请求头 / 查询串里取密钥（头优先）。
pub fn key_from_request(headers: &HeaderMap, query: &HashMap<String, String>) -> Option<String> {
    for name in ["x-request-key", "x-lxm-auth", "x-lx-pass", "authorization"] {
        if let Some(value) = headers.get(name).and_then(|v| v.to_str().ok()) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.trim_start_matches("Bearer ").to_string());
            }
        }
    }
    query.get("key").or_else(|| query.get("pass")).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// 定长比较，避免按字节提前返回泄露前缀。
pub fn constant_time_eq(left: &str, right: &str) -> bool {
    let (a, b) = (left.as_bytes(), right.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// 密钥是否通过。未开启密钥时一律放行。
pub fn verify_key(cfg: &Config, key: Option<&str>) -> bool {
    if !cfg.auth.api_pass.enable && !cfg.auth.uni_pass.enable {
        return true;
    }
    let Some(key) = key else { return false };
    if cfg.auth.api_pass.enable && !cfg.auth.api_pass.value.is_empty() && constant_time_eq(key, &cfg.auth.api_pass.value) {
        return true;
    }
    if cfg.auth.uni_pass.enable && cfg.auth.uni_pass.pool.contains_key(key) {
        return true;
    }
    // 开了密钥但没配值：视为「谁都能进」，否则用户会被自己锁死
    cfg.auth.api_pass.enable && cfg.auth.api_pass.value.is_empty() && cfg.auth.uni_pass.pool.is_empty()
}

/// 密钥对应的可用平台（未启用独立密钥时为 `None` = 不限平台）。
pub fn allowed_platforms<'a>(cfg: &'a Config, key: Option<&'a str>) -> Option<&'a Vec<String>> {
    if !cfg.auth.uni_pass.enable {
        return None;
    }
    key.and_then(|k| cfg.auth.uni_pass.pool.get(k))
}

/// 解析 `1.2.3.4` / `1.2.3.0/24` / `::1`，返回 (网段基址, 前缀长度)。
fn parse_cidr(entry: &str) -> Option<(IpAddr, u8)> {
    let entry = entry.trim();
    if entry.is_empty() {
        return None;
    }
    match entry.split_once('/') {
        Some((ip, prefix)) => {
            let ip: IpAddr = ip.trim().parse().ok()?;
            let prefix: u8 = prefix.trim().parse().ok()?;
            Some((ip, prefix))
        }
        None => {
            let ip: IpAddr = entry.parse().ok()?;
            let bits = if ip.is_ipv4() { 32 } else { 128 };
            Some((ip, bits))
        }
    }
}

fn cidr_match(ip: IpAddr, base: IpAddr, prefix: u8) -> bool {
    match (ip, base) {
        (IpAddr::V4(ip), IpAddr::V4(base)) => {
            if prefix == 0 {
                return true;
            }
            let mask = u32::MAX << (32 - prefix.min(32));
            (u32::from(ip) & mask) == (u32::from(base) & mask)
        }
        (IpAddr::V6(ip), IpAddr::V6(base)) => {
            if prefix == 0 {
                return true;
            }
            let mask = u128::MAX << (128 - prefix.min(128));
            (u128::from(ip) & mask) == (u128::from(base) & mask)
        }
        _ => false,
    }
}

fn list_match(ip: IpAddr, list: &[String]) -> bool {
    list.iter().any(|entry| match parse_cidr(entry) {
        Some((base, prefix)) => cidr_match(ip, base, prefix),
        None => false,
    })
}

/// IP 是否放行。`strict` 时白名单之外一律拒绝，否则只拒黑名单。
pub fn ip_allowed(cfg: &Config, ip: IpAddr) -> bool {
    let ban = &cfg.auth.ban_list;
    if !ban.enable {
        return true;
    }
    if list_match(ip, &ban.black) {
        return false;
    }
    if ban.strict {
        return list_match(ip, &ban.white);
    }
    true
}

/// 固定窗口计数 + 超限封禁。
pub struct RateLimiter {
    window: Mutex<RateState>,
    /// 全局窗口（每秒）
    global: Mutex<(Instant, u32)>,
}

#[derive(Default)]
struct RateState {
    /// ip -> (窗口起点, 次数)
    per_ip: HashMap<IpAddr, (Instant, u32)>,
    /// ip -> 解封时间
    banned: HashMap<IpAddr, Instant>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self { window: Mutex::new(RateState::default()), global: Mutex::new((Instant::now(), 0)) }
    }

    /// 是否放行；`cfg` 未启用时恒为 true。
    pub fn allow(&self, cfg: &Config, ip: IpAddr) -> bool {
        let rl = &cfg.auth.rate_limit;
        if !rl.enable {
            return true;
        }
        let now = Instant::now();
        // 全局每秒上限
        if rl.globle > 0 {
            let mut global = match self.global.lock() {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };
            if now.duration_since(global.0) >= Duration::from_secs(1) {
                *global = (now, 0);
            }
            if global.1 >= rl.globle {
                return false;
            }
            global.1 += 1;
        }
        let mut state = match self.window.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        if let Some(until) = state.banned.get(&ip) {
            if *until > now {
                return false;
            }
            state.banned.remove(&ip);
        }
        let entry = state.per_ip.entry(ip).or_insert((now, 0));
        if now.duration_since(entry.0) >= Duration::from_secs(60) {
            *entry = (now, 0);
        }
        entry.1 += 1;
        if rl.single > 0 && entry.1 > rl.single {
            if rl.ban_num == 0 || entry.1 > rl.single + rl.ban_num {
                state.banned.insert(ip, now + Duration::from_secs(rl.ban_tim.max(1)));
            }
            return false;
        }
        true
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}
