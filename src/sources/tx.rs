//! QQ 音乐（`tx`）。`musicu.fcg` 换 vkey；歌词走 `fcg_query_lyric_new`；封面地址是确定性的。

use serde_json::Value;

use crate::{cache::now_secs, itype::{Album, Artist, FileItem, Link, LyricInfo, MusicInfo, SearchPage}, state::AppState};

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36";
const TTL: u64 = 1800;

fn filename_for(qual: &str, mid: &str) -> String {
    match qual {
        "flac24bit" => format!("RS01{mid}.flac"),
        "flac" => format!("F000{mid}.flac"),
        "320k" => format!("M800{mid}.mp3"),
        _ => format!("M500{mid}.mp3"),
    }
}

pub async fn url(st: &AppState, id: &str, qual: &str) -> Result<Link, String> {
    let filename = filename_for(qual, id);
    let body = serde_json::json!({
        "req_0": {
            "module": "vkey.GetVkeyServer",
            "method": "CgiGetVkey",
            "param": {
                "guid": "10000", "songmid": [id], "songtype": [0], "uin": "0",
                "loginflag": 1, "platform": "20", "filename": [filename],
            }
        },
        "loginUin": "0",
        "comm": { "uin": "0", "format": "json", "ct": 24, "cv": 0 },
    });
    let json: Value = st
        .http
        .post("https://u.y.qq.com/cgi-bin/musicu.fcg")
        .query(&[("format", "json"), ("data", &body.to_string())])
        .header("User-Agent", UA)
        .header("Referer", "https://y.qq.com/")
        .send()
        .await
        .map_err(|e| format!("QQ 请求失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("QQ 解析失败: {e}"))?;
    let data = json.get("req_0").and_then(|r| r.get("data")).ok_or_else(|| "QQ 未返回数据".to_string())?;
    let info = data
        .get("midurlinfo")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .ok_or_else(|| "QQ 未返回 vkey".to_string())?;
    let purl = info.get("purl").and_then(Value::as_str).unwrap_or_default();
    if purl.is_empty() {
        return Err(format!("QQ 未下发 vkey（result={}）", info.get("result").and_then(Value::as_i64).unwrap_or(-1)));
    }
    let sip = data
        .get("sip")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("http://ws.stream.qqmusic.qq.com/");
    Ok(Link { url: format!("{sip}{purl}"), qual: qual.to_string(), expi: now_secs() + TTL, size: None })
}

pub async fn lyric(st: &AppState, id: &str) -> Result<LyricInfo, String> {
    let text = st
        .http
        .get("https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg")
        .query(&[("songmid", id), ("format", "json"), ("nobase64", "1"), ("g_tk", "5381")])
        .header("User-Agent", UA)
        .header("Referer", "https://y.qq.com/portal/player.html")
        .send()
        .await
        .map_err(|e| format!("QQ 歌词请求失败: {e}"))?
        .text()
        .await
        .unwrap_or_default();
    let trimmed = text.trim().trim_start_matches("MusicJsonCallback(").trim_end_matches(')');
    let json: Value = serde_json::from_str(trimmed).map_err(|_| "QQ 歌词不是合法 JSON".to_string())?;
    let lyric = json.get("lyric").and_then(Value::as_str).unwrap_or_default();
    if lyric.is_empty() {
        return Err("QQ 没有歌词".to_string());
    }
    let translated = json.get("trans").and_then(Value::as_str).unwrap_or_default();
    Ok(LyricInfo {
        lyric: lyric.to_string(),
        tlyric: if translated.is_empty() { String::new() } else { crate::lyric::normalize_lrc(translated) },
        ..Default::default()
    })
}

pub async fn pic(_st: &AppState, id: &str, size: &str) -> Result<String, String> {
    // 入参是 albumMid；150/300/500/800 是 QQ 官方支持的档位
    let px = match size {
        "150" => "150",
        "500" => "500",
        "800" => "800",
        _ => "300",
    };
    Ok(format!("https://y.gtimg.cn/music/photo_new/T002R{px}x{px}M000{id}.jpg"))
}

pub async fn info(st: &AppState, id: &str) -> Result<MusicInfo, String> {
    let json: Value = st
        .http
        .get("https://c.y.qq.com/v8/fcg-bin/fcg_play_single_song.fcg")
        .query(&[("songmid", id), ("format", "json"), ("platform", "yqq"), ("needNewCode", "1")])
        .header("User-Agent", UA)
        .header("Referer", "https://y.qq.com/")
        .send()
        .await
        .map_err(|e| format!("QQ 信息请求失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("QQ 信息解析失败: {e}"))?;
    let item = json.get("data").and_then(Value::as_array).and_then(|a| a.first()).ok_or_else(|| "QQ 没有歌曲信息".to_string())?;
    let singers = item.get("singer").and_then(Value::as_array).cloned().unwrap_or_default();
    let album_mid = item.get("album").and_then(|a| a.get("mid")).and_then(Value::as_str).unwrap_or_default().to_string();
    Ok(MusicInfo {
        id: id.to_string(),
        name: item.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
        alia: None,
        ar: singers
            .iter()
            .map(|s| Artist { id: s.get("mid").and_then(Value::as_str).unwrap_or_default().to_string(), name: s.get("name").and_then(Value::as_str).unwrap_or_default().to_string(), alia: None })
            .collect(),
        al: Album {
            id: album_mid.clone(),
            name: item.get("album").and_then(|a| a.get("name")).and_then(Value::as_str).unwrap_or_default().to_string(),
            alia: None,
            covr: if album_mid.is_empty() { String::new() } else { format!("https://y.gtimg.cn/music/photo_new/T002R300x300M000{album_mid}.jpg") },
        },
        dt: item.get("interval").and_then(Value::as_u64).unwrap_or(0) as u32,
        mv: item.get("mv").and_then(|m| m.get("vid")).and_then(Value::as_str).unwrap_or_default().to_string(),
        fee: item.get("pay").and_then(|p| p.get("pay_play")).and_then(Value::as_u64).unwrap_or(0) as u32,
        file: vec![FileItem { qual: "320k".into(), size: None, sizs: None, hash: Some(item.get("file").and_then(|f| f.get("media_mid")).and_then(Value::as_str).unwrap_or_default().to_string()) }],
    })
}

pub async fn search(st: &AppState, keyword: &str, page: u32, size: u32) -> Result<SearchPage, String> {
    let json: Value = st
        .http
        .get("https://c.y.qq.com/soso/fcgi-bin/search_for_qq_cp")
        .query(&[
            ("g_tk", "5381"), ("uin", "0"), ("format", "json"), ("inCharset", "utf-8"), ("outCharset", "utf-8"),
            ("notice", "0"), ("platform", "h5"), ("needNewCode", "1"), ("w", keyword), ("t", "0"),
            ("flag", "1"), ("ie", "utf-8"), ("aggr", "0"), ("perpage", &size.to_string()),
            ("n", &size.to_string()), ("p", &page.to_string()), ("remoteplace", "txt.mqq.all"),
        ])
        .header("User-Agent", UA)
        .header("Referer", "https://y.qq.com/")
        .send()
        .await
        .map_err(|e| format!("QQ 搜索失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("QQ 搜索解析失败: {e}"))?;
    let song = json.get("data").and_then(|d| d.get("song")).cloned().unwrap_or(Value::Null);
    let total = song.get("totalnum").and_then(Value::as_u64).unwrap_or(0);
    let mut items = Vec::new();
    if let Some(list) = song.get("list").and_then(Value::as_array) {
        for item in list {
            let mid = item.get("mid").and_then(Value::as_str).unwrap_or_default().to_string();
            if mid.is_empty() {
                continue;
            }
            let album_mid = item.get("album").and_then(|a| a.get("mid")).and_then(Value::as_str).unwrap_or_default().to_string();
            items.push(MusicInfo {
                id: mid,
                name: item.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
                alia: None,
                ar: item
                    .get("singer")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().map(|s| Artist { id: String::new(), name: s.get("name").and_then(Value::as_str).unwrap_or_default().to_string(), alia: None }).collect())
                    .unwrap_or_default(),
                al: Album {
                    id: album_mid.clone(),
                    name: item.get("album").and_then(|a| a.get("name")).and_then(Value::as_str).unwrap_or_default().to_string(),
                    alia: None,
                    covr: if album_mid.is_empty() { String::new() } else { format!("https://y.gtimg.cn/music/photo_new/T002R300x300M000{album_mid}.jpg") },
                },
                dt: item.get("interval").and_then(Value::as_u64).unwrap_or(0) as u32,
                mv: String::new(),
                fee: 0,
                file: vec![],
            });
        }
    }
    Ok(SearchPage { total, items })
}
