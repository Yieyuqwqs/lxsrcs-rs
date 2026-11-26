//! 网易云（`wy`）。走官方 weapi/eapi，无需登录：
//! 320k 对「试听/免费」曲目匿名可取，flac 需要会员——取不到就如实降档。
//!
//! 实测（2026-11-26，本机）：`song/enhance/player/url/v1` + `level=exhigh` 直接回 320k 真链；
//! `level=lossless` 对无会员曲目回 320k（`br=320001`），因此以响应里的 `br/type` 判定实际音质。

use serde_json::Value;

use crate::{
    cache::now_secs,
    crypto,
    itype::{Album, Artist, FileItem, Link, LyricInfo, MusicInfo, SearchPage},
    state::AppState,
};

const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/60.0.3112.90 Safari/537.36";
const TTL: u64 = 20 * 60;

fn level_for(qual: &str) -> &'static str {
    match qual {
        "192k" => "higher",
        "320k" => "exhigh",
        "flac" => "lossless",
        "flac24bit" | "hires" => "hires",
        _ => "standard",
    }
}

async fn weapi(st: &AppState, path: &str, body: &Value, referer: &str) -> Result<Value, String> {
    let form = crypto::weapi_form(body);
    let resp = st
        .http
        .post(format!("https://music.163.com/weapi/{path}"))
        .header("User-Agent", UA)
        .header("Referer", referer)
        .header("origin", "https://music.163.com")
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("网易云请求失败: {e}"))?;
    let text = resp.text().await.map_err(|e| format!("网易云响应读取失败: {e}"))?;
    serde_json::from_str(&text).map_err(|_| format!("网易云返回非 JSON: {}", &text[..text.len().min(120)]))
}

async fn eapi(st: &AppState, path: &str, body: &Value) -> Result<Value, String> {
    let form = crypto::eapi_form(path, body);
    let resp = st
        .http
        .post(format!("https://interface3.music.163.com/eapi{path}"))
        .header("User-Agent", UA)
        .header("Referer", "https://music.163.com/")
        .header("origin", "https://music.163.com")
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("网易云 eapi 请求失败: {e}"))?;
    let text = resp.text().await.map_err(|e| format!("网易云 eapi 响应读取失败: {e}"))?;
    serde_json::from_str(&text).map_err(|_| format!("网易云 eapi 返回非 JSON: {}", &text[..text.len().min(120)]))
}

pub async fn url(st: &AppState, id: &str, qual: &str) -> Result<Link, String> {
    let level = level_for(qual);
    let body = serde_json::json!({ "ids": format!("[{id}]"), "level": level, "encodeType": "flac", "br": 999000 });
    let json = weapi(st, "song/enhance/player/url/v1", &body, "https://music.163.com/").await?;
    let code = json.get("code").and_then(Value::as_i64).unwrap_or(-1);
    if code != 200 {
        return Err(format!("网易云返回 code={code}"));
    }
    let item = json
        .get("data")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .ok_or_else(|| "网易云没有返回数据".to_string())?;
    let link = item.get("url").and_then(Value::as_str).unwrap_or_default();
    if link.is_empty() || link == "null" {
        return Err("网易云未下发播放链接（多为版权/会员限制）".to_string());
    }
    let br = item.get("br").and_then(Value::as_u64).unwrap_or(0);
    let typ = item.get("type").and_then(Value::as_str).unwrap_or("mp3");
    let actual = match (br, typ) {
        (b, "flac") if b >= 1_500_000 => "flac24bit",
        (b, "flac") if b >= 900_000 => "flac",
        (b, _) if b >= 300_000 => "320k",
        (b, _) if b >= 190_000 => "192k",
        _ => "128k",
    };
    Ok(Link { url: link.to_string(), qual: actual.to_string(), expi: now_secs() + TTL, size: item.get("size").and_then(Value::as_u64) })
}

/// 逐行解析网易云的 yrc（逐字歌词）：`{"t":毫秒,"c":[{"tx":"字"}]}` 一行一条。
fn yrc_to_lxlyric(yrc: &str) -> String {
    let mut out = String::new();
    for line in yrc.lines() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
        let start = value.get("t").and_then(Value::as_u64).unwrap_or(0);
        let text: String = value
            .get("c")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(|c| c.get("tx").and_then(Value::as_str)).collect())
            .unwrap_or_default();
        if text.trim().is_empty() {
            continue;
        }
        let m = start / 60000;
        let s = (start % 60000) / 1000;
        let ms = start % 1000;
        out.push_str(&format!("[{m:02}:{s:02}.{ms:03}]<0,500>{text}\n"));
    }
    out
}

pub async fn lyric(st: &AppState, id: &str) -> Result<LyricInfo, String> {
    let body = serde_json::json!({ "id": id, "cp": false, "tv": 0, "lv": 0, "rv": 0, "kv": 0, "yv": 0, "ytv": 0, "yrv": 0 });
    let json = eapi(st, "/api/song/lyric/v1", &body).await?;
    if json.get("code").and_then(Value::as_i64) != Some(200) {
        return Err("网易云歌词获取失败".to_string());
    }
    let pick = |key: &str| json.get(key).and_then(|v| v.get("lyric")).and_then(Value::as_str).unwrap_or_default().to_string();
    let lyric = pick("lrc");
    if lyric.is_empty() {
        return Err("网易云没有歌词".to_string());
    }
    let yrc = json.get("yrc").and_then(|v| v.get("lyric")).and_then(Value::as_str).unwrap_or_default();
    let lxlyric = if yrc.is_empty() { String::new() } else { yrc_to_lxlyric(yrc) };
    Ok(LyricInfo { lyric, tlyric: pick("tlyric"), rlyric: pick("romalrc"), lxlyric })
}

fn size_param(size: &str) -> Option<u32> {
    match size {
        "" | "l" | "large" => Some(1024),
        "s" | "small" => Some(256),
        "m" | "middle" => Some(512),
        _ => size.parse::<u32>().ok(),
    }
}

pub async fn pic(st: &AppState, id: &str, size: &str) -> Result<String, String> {
    let info = info(st, id).await?;
    if info.al.covr.is_empty() {
        return Err("网易云没有封面".to_string());
    }
    Ok(match size_param(size) {
        Some(px) => format!("{}?param={px}y{px}", info.al.covr),
        None => info.al.covr,
    })
}

fn parse_detail_item(item: &Value) -> Option<MusicInfo> {
    let id = item.get("id").and_then(Value::as_i64).unwrap_or(0).to_string();
    let name = item.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
    if id == "0" || name.is_empty() {
        return None;
    }
    let aliases = item.get("alia").and_then(Value::as_array).cloned().unwrap_or_default();
    let ar = item
        .get("ar")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|a| Artist {
                    id: a.get("id").and_then(Value::as_i64).unwrap_or(0).to_string(),
                    name: a.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
                    alia: None,
                })
                .collect()
        })
        .unwrap_or_default();
    let al_value = item.get("al").cloned().unwrap_or(Value::Null);
    let al = Album {
        id: al_value.get("id").and_then(Value::as_i64).unwrap_or(0).to_string(),
        name: al_value.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
        alia: None,
        covr: al_value.get("picUrl").and_then(Value::as_str).unwrap_or_default().to_string(),
    };
    // 各档位文件大小：h/m/l/sq/hr
    let mut file = Vec::new();
    for (key, qual) in [("l", "128k"), ("m", "192k"), ("h", "320k"), ("sq", "flac"), ("hr", "flac24bit")] {
        if let Some(size) = item.get(key).and_then(|v| v.get("size")).and_then(Value::as_u64) {
            if size > 0 {
                file.push(FileItem { qual: qual.to_string(), size: Some(size), sizs: None, hash: None });
            }
        }
    }
    Some(MusicInfo {
        id,
        name,
        alia: if aliases.is_empty() { None } else { Some(aliases.iter().filter_map(Value::as_str).map(str::to_string).collect()) },
        ar,
        al,
        dt: item.get("dt").and_then(Value::as_u64).unwrap_or(0) as u32 / 1000,
        mv: item.get("mv").and_then(Value::as_i64).unwrap_or(0).to_string(),
        fee: item.get("fee").and_then(Value::as_u64).unwrap_or(0) as u32,
        file,
    })
}

pub async fn info(st: &AppState, id: &str) -> Result<MusicInfo, String> {
    let body = serde_json::json!({ "c": format!("[{{\"id\":{id}}}]"), "ids": format!("[{id}]") });
    let json = weapi(st, "v3/song/detail", &body, &format!("https://music.163.com/song?id={id}")).await?;
    if json.get("code").and_then(Value::as_i64) != Some(200) {
        return Err("网易云歌曲信息获取失败".to_string());
    }
    let item = json.get("songs").and_then(Value::as_array).and_then(|a| a.first()).ok_or_else(|| "网易云歌曲不存在".to_string())?;
    parse_detail_item(item).ok_or_else(|| "网易云歌曲信息解析失败".to_string())
}

pub async fn search(st: &AppState, keyword: &str, page: u32, size: u32) -> Result<SearchPage, String> {
    let limit = size.clamp(1, 100);
    let offset = page.saturating_sub(1) * limit;
    let body = serde_json::json!({ "s": keyword, "limit": limit, "offset": offset, "type": 1 });
    // `cloudsearch` 不带 csrf 会回 50000005，实测 `search/suggest/web` 匿名可用
    let json = weapi(st, "search/suggest/web", &body, "https://music.163.com/search/").await?;
    let result = json.get("result").cloned().unwrap_or(Value::Null);
    let items: Vec<MusicInfo> = result
        .get("songs")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(parse_detail_item).collect())
        .unwrap_or_default();
    let total = result.get("songCount").and_then(Value::as_u64).unwrap_or(items.len() as u64);
    Ok(SearchPage { total, items })
}
