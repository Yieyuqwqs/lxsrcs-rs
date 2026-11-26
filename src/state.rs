//! 进程内共享状态：配置、HTTP 客户端（连接池）、两级缓存、统计、限流。

use std::{net::IpAddr, sync::atomic::{AtomicU64, Ordering}, sync::Arc, time::Instant};

use crate::auth::RateLimiter;
use crate::cache::{now_secs, FileCache, MemCache};
use crate::config::Config;

pub struct PlatformCount {
    pub request: AtomicU64,
    pub success: AtomicU64,
}

impl Default for PlatformCount {
    fn default() -> Self {
        Self { request: AtomicU64::new(0), success: AtomicU64::new(0) }
    }
}

pub struct Stats {
    pub start_at: u64,
    pub access: AtomicU64,
    pub request: AtomicU64,
    pub success: AtomicU64,
    /// wy / mg / kw / kg / tx / bi / lx
    pub per: [PlatformCount; 7],
}

impl Stats {
    pub fn new() -> Self {
        Self {
            start_at: now_secs(),
            access: AtomicU64::new(0),
            request: AtomicU64::new(0),
            success: AtomicU64::new(0),
            per: Default::default(),
        }
    }

    pub fn platform_index(plat: &str) -> Option<usize> {
        match plat {
            "wy" => Some(0),
            "mg" => Some(1),
            "kw" => Some(2),
            "kg" => Some(3),
            "tx" => Some(4),
            "bi" => Some(5),
            "lx" => Some(6),
            _ => None,
        }
    }

    pub fn bump_request(&self, plat: &str) {
        self.request.fetch_add(1, Ordering::Relaxed);
        if let Some(i) = Self::platform_index(plat) {
            self.per[i].request.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn bump_success(&self, plat: &str) {
        self.success.fetch_add(1, Ordering::Relaxed);
        if let Some(i) = Self::platform_index(plat) {
            self.per[i].success.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn bump_access(&self) {
        self.access.fetch_add(1, Ordering::Relaxed);
    }

    pub fn summary(&self) -> serde_json::Value {
        let mut count = serde_json::Map::new();
        for (i, plat) in ["wy", "mg", "kw", "kg", "tx", "bi", "lx"].iter().enumerate() {
            count.insert(
                plat.to_string(),
                serde_json::json!([
                    self.per[i].request.load(Ordering::Relaxed),
                    self.per[i].success.load(Ordering::Relaxed)
                ]),
            );
        }
        serde_json::json!({
            "StartAt": self.start_at,
            "Accessn": self.access.load(Ordering::Relaxed),
            "Request": self.request.load(Ordering::Relaxed),
            "Success": self.success.load(Ordering::Relaxed),
            "count": serde_json::Value::Object(count),
        })
    }
}

impl Default for Stats {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AppState {
    pub cfg: Config,
    pub http: reqwest::Client,
    pub mem: MemCache,
    pub file: FileCache,
    pub stats: Stats,
    pub limiter: RateLimiter,
    pub start: Instant,
    /// 内置的自定义源脚本，启动时把地址/密钥注入好
    pub script: String,
    /// 错误音频（1 秒静音 mp3），`Errs.ErrMp3` 为空时用它兜底
    pub silence_mp3: Vec<u8>,
    pub version: String,
}

impl AppState {
    pub fn new(cfg: Config) -> Arc<Self> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .connect_timeout(std::time::Duration::from_secs(8))
            .user_agent(crate::util::default_ua())
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let mem = MemCache::new(cfg.memo.shards);
        let file = FileCache::new(&cfg.data.local.path);
        let stats = Stats::new();
        let limiter = RateLimiter::new();
        let script = String::from(include_str!("../assets/script.js"));
        let silence_mp3 = crate::util::silence_mp3();

        Arc::new(Self {
            cfg,
            http,
            mem,
            file,
            stats,
            limiter,
            start: Instant::now(),
            script,
            silence_mp3,
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    /// 错误音频地址：优先用配置，其次用本机 `/api/v1/file/er/error.mp3`。
    pub fn err_audio(&self, host: &str) -> String {
        if !self.cfg.errs.err_mp3.is_empty() {
            return self.cfg.errs.err_mp3.clone();
        }
        let sign = crate::util::error_sign(&self.cfg, now_secs());
        format!("{host}/api/v1/file/er/error.mp3?sign={sign}")
    }

    pub fn client_ip(&self, headers: &axum::http::HeaderMap, fallback: Option<IpAddr>) -> IpAddr {
        if !self.cfg.main.real_ip.is_empty() {
            for name in [self.cfg.main.real_ip.as_str(), "x-forwarded-for", "x-real-ip"] {
                if let Some(value) = headers.get(name).and_then(|v| v.to_str().ok()) {
                    let first = value.split(',').next().unwrap_or("").trim();
                    if let Ok(ip) = first.parse::<IpAddr>() {
                        return ip;
                    }
                }
            }
        }
        fallback.unwrap_or(IpAddr::from([127, 0, 0, 1]))
    }
}
