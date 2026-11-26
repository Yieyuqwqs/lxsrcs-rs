//! B 站（`bi`）：`view` 取 cid，`playurl` 取音频流。需要在配置里给 CDN 时改写成镜像。

use serde_json::Value;

use crate::{cache::now_secs, itype::{Album, Artist, Link, MusicInfo, SearchPage}, state::AppState};

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36";

async fn view(st: &AppState, id: &str) -> Result<Value, String> {
    let query = if id.to_lowercase().starts_with("bv") { vec![("bvid", id)] } else { vec![("aid", id)] };
    let json: Value = st
        .http
        .get("https://api.bilibili.com/x/web-interface/view")
        .query(&query)
        .header("User-Agent", UA)
        .header("Referer", "https://www.bilibili.com/")
        .send()
        .await
        .map_err(|e| format!("B站请求失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("B站解析失败: {e}"))?;
    if json.get("code").and_then(Value::as_i64) != Some(0) {
        return Err(format!("B站返回 code={}", json.get("code").and_then(Value::as_i64).unwrap_or(-1)));
    }
    json.get("data").cloned().ok_or_else(|| "B站没有返回数据".to_string())
}

pub async fn url(st: &AppState, id: &str, _qual: &str) -> Result<Link, String> {
    let data = view(st, id).await?;
    let cid = data.get("cid").and_then(Value::as_u64).unwrap_or(0);
    let bvid = data.get("bvid").and_then(Value::as_str).unwrap_or_default();
    let json: Value = st
        .http
        .get("https://api.bilibili.com/x/player/playurl")
        .query(&[("bvid", bvid), ("cid", &cid.to_string()), ("qn", "80"), ("fnval", "16"), ("fourk", "0")])
        .header("User-Agent", UA)
        .header("Referer", "https://www.bilibili.com/")
        .send()
        .await
        .map_err(|e| format!("B站取流失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("B站取流解析失败: {e}"))?;
    let dash = json.get("data").and_then(|d| d.get("dash")).ok_or_else(|| "B站没有 dash 音轨（可能需要登录）".to_string())?;
    let best = dash
        .get("audio")
        .and_then(Value::as_array)
        .and_then(|a| a.iter().max_by_key(|x| x.get("bandwidth").and_then(Value::as_u64).unwrap_or(0)))
        .ok_or_else(|| "B站没有音频流".to_string())?;
    let base = best.get("baseUrl").or_else(|| best.get("base_url")).and_then(Value::as_str).unwrap_or_default();
    if base.is_empty() {
        return Err("B站音频流地址为空".to_string());
    }
    Ok(Link { url: base.to_string(), qual: "orig".to_string(), expi: now_secs() + 3600, size: None })
}

pub async fn pic(st: &AppState, id: &str, _size: &str) -> Result<String, String> {
    let data = view(st, id).await?;
    Ok(data.get("pic").and_then(Value::as_str).unwrap_or_default().to_string())
}

pub async fn info(st: &AppState, id: &str) -> Result<MusicInfo, String> {
    let data = view(st, id).await?;
    let owner = data.get("owner").cloned().unwrap_or(Value::Null);
    Ok(MusicInfo {
        id: id.to_string(),
        name: data.get("title").and_then(Value::as_str).unwrap_or_default().to_string(),
        alia: None,
        ar: vec![Artist { id: owner.get("mid").and_then(Value::as_u64).unwrap_or(0).to_string(), name: owner.get("name").and_then(Value::as_str).unwrap_or_default().to_string(), alia: None }],
        al: Album { id: String::new(), name: "Bilibili".into(), alia: None, covr: data.get("pic").and_then(Value::as_str).unwrap_or_default().to_string() },
        dt: data.get("duration").and_then(Value::as_u64).unwrap_or(0) as u32,
        mv: data.get("bvid").and_then(Value::as_str).unwrap_or_default().to_string(),
        fee: 0,
        file: vec![],
    })
}

pub async fn search(_st: &AppState, _keyword: &str, _page: u32, _size: u32) -> Result<SearchPage, String> {
    Err("B站搜索需要 wbi 签名，暂未实现".to_string())
}
