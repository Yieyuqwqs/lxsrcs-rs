//! 两级缓存：内存分片（秒级 TTL）+ 本地文件（跨重启，带 expi）。
//!
//! key 一律用「方法/平台/ID/音质」拼出的稳定串，值与 Go 版一致：命中的是**直链字符串**。

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
struct Entry {
    value: String,
    expire: Instant,
}

/// 分片内存缓存：`Memo.Shards` 片，每片一把锁，减少锁竞争。
pub struct MemCache {
    shards: Vec<Mutex<HashMap<String, Entry>>>,
}

impl MemCache {
    pub fn new(shards: usize) -> Self {
        let n = shards.clamp(1, 4096);
        let mut shards = Vec::with_capacity(n);
        for _ in 0..n {
            shards.push(Mutex::new(HashMap::new()));
        }
        Self { shards }
    }

    fn shard(&self, key: &str) -> &Mutex<HashMap<String, Entry>> {
        // 简单 FNV-1a，稳定且够散
        let mut hash: u64 = 0xcbf29ce484222325;
        for b in key.as_bytes() {
            hash ^= *b as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        &self.shards[(hash as usize) % self.shards.len()]
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let mut shard = match self.shard(key).lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        match shard.get(key) {
            Some(entry) if entry.expire > Instant::now() => Some(entry.value.clone()),
            Some(_) => {
                shard.remove(key);
                None
            }
            None => None,
        }
    }

    pub fn set(&self, key: &str, value: &str, ttl: u64) {
        let mut shard = match self.shard(key).lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        shard.insert(
            key.to_string(),
            Entry { value: value.to_string(), expire: Instant::now() + Duration::from_secs(ttl.max(1)) },
        );
        // 顺手做一次轻量回收，避免长期运行内存只涨不落
        if shard.len() > 4096 {
            let now = Instant::now();
            shard.retain(|_, entry| entry.expire > now);
        }
    }
}

/// 本地缓存：每条记录一个 json 文件，`expi` 是绝对时间戳（与对外字段一致）。
pub struct FileCache {
    dir: PathBuf,
}

impl FileCache {
    pub fn new(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref().to_path_buf();
        let _ = std::fs::create_dir_all(&dir);
        Self { dir }
    }

    fn path_of(&self, key: &str) -> PathBuf {
        let name = format!("{:x}", md5::compute(key.as_bytes()));
        self.dir.join(format!("{name}.json"))
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let text = std::fs::read_to_string(self.path_of(key)).ok()?;
        let value: serde_json::Value = serde_json::from_str(&text).ok()?;
        let expi = value.get("expi")?.as_u64()?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
        if expi <= now {
            let _ = std::fs::remove_file(self.path_of(key));
            return None;
        }
        value.get("url")?.as_str().map(|s| s.to_string())
    }

    pub fn set(&self, key: &str, url: &str, expi: u64) {
        let record = serde_json::json!({ "url": url, "expi": expi });
        let _ = std::fs::write(self.path_of(key), record.to_string());
    }
}

/// 当前 unix 秒。
pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
