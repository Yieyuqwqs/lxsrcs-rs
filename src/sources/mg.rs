//! 咪咕（`mg`）。`listenSong.do` 用 302 跳转到真实 CDN；匿名只有 128k。

use serde_json::Value;

use crate::{cache::now_secs, itype::{Album, Artist, Link, LyricInfo, MusicInfo, SearchPage}, state::AppState};

const UA: &str = "Mozilla/5.0 (Linux; Android 12) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120 Mobile Safari/537.36";
const TTL: u64 = 24 * 60 * 60;

fn tone_for(qual: &str) -> &'static str {
    match qual {
        "flac" => "SQ",
        "flac24bit" => "ZQ",
        "320k" => "HQ",
        _ => "PQ",
    }
}

pub async fn url(st: &AppState, id: &str, qual: &str) -> Result<Link, String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("咪咕客户端构建失败: {e}"))?;
    let resp = client
        .get("https://app.c.nf.migu.cn/MIGUM2.0/v1.0/content/sub/listenSong.do")
        .query(&[
            ("toneFlag", tone_for(qual)),
            ("netType", "00"),
            ("userId", "1554861458870011"),
            ("ua", "Android_migu"),
            ("version", "5.0.9"),
            ("mediaId", id),
            ("copyrightId", id),
            ("contentId", id),
            ("resourceType", "2"),
            ("channel", "0"),
        ])
        .header("User-Agent", UA)
        .send()
        .await
        .map_err(|e| format!("咪咕请求失败: {e}"))?;
    let location = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .ok_or_else(|| format!("咪咕未下发跳转（HTTP {}）", resp.status().as_u16()))?;
    let actual = if location.contains("flac") { "flac" } else if location.contains("320") { "320k" } else { "128k" };
    Ok(Link { url: location, qual: actual.to_string(), expi: now_secs() + TTL, size: None })
}

pub async fn lyric(st: &AppState, id: &str) -> Result<LyricInfo, String> {
    let json: Value = st
        .http
        .get("https://music.migu.cn/v3/api/music/audioPlayer/getLyric")
        .query(&[("copyrightId", id)])
        .header("User-Agent", UA)
        .header("Referer", "https://music.migu.cn/")
        .send()
        .await
        .map_err(|e| format!("咪咕歌词请求失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("咪咕歌词解析失败: {e}"))?;
    let text = json.get("lyric").and_then(Value::as_str).unwrap_or_default();
    if text.trim().is_empty() {
        return Err("咪咕没有歌词".to_string());
    }
    Ok(crate::lyric::split_lrc(text))
}

pub async fn pic(st: &AppState, id: &str, _size: &str) -> Result<String, String> {
    let info = info(st, id).await?;
    if info.al.covr.is_empty() {
        return Err("咪咕没有封面".to_string());
    }
    Ok(info.al.covr)
}

pub async fn info(st: &AppState, id: &str) -> Result<MusicInfo, String> {
    let json: Value = st
        .http
        .post("https://c.musicapp.migu.cn/MIGUM2.0/v1.0/content/resourceinfo.do")
        .query(&[("resourceType", "2")])
        .form(&[("resourceId", id)])
        .header("User-Agent", UA)
        .send()
        .await
        .map_err(|e| format!("咪咕信息请求失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("咪咕信息解析失败: {e}"))?;
    let item = json.get("resource").and_then(Value::as_array).and_then(|a| a.first()).ok_or_else(|| "咪咕没有歌曲信息".to_string())?;
    Ok(MusicInfo {
        id: id.to_string(),
        name: item.get("songName").and_then(Value::as_str).unwrap_or_default().to_string(),
        alia: None,
        ar: item
            .get("singerList")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|s| Artist { id: String::new(), name: s.get("name").and_then(Value::as_str).unwrap_or_default().to_string(), alia: None }).collect())
            .unwrap_or_default(),
        al: Album {
            id: String::new(),
            name: item.get("album").and_then(Value::as_str).unwrap_or_default().to_string(),
            alia: None,
            covr: item.get("img1").or_else(|| item.get("img2")).or_else(|| item.get("img3")).and_then(Value::as_str).unwrap_or_default().to_string(),
        },
        dt: item.get("duration").and_then(Value::as_str).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0) / 1000,
        mv: item.get("mvId").and_then(Value::as_str).unwrap_or_default().to_string(),
        fee: 0,
        file: vec![],
    })
}

pub async fn search(st: &AppState, keyword: &str, page: u32, size: u32) -> Result<SearchPage, String> {
    let json: Value = st
        .http
        .get("https://c.musicapp.migu.cn/MIGUM2.0/v1.0/content/search_all.do")
        .query(&[
            ("text", keyword),
            ("pageNo", &page.to_string()),
            ("pageSize", &size.to_string()),
            ("searchSwitch", "{\"song\":1}"),
            ("isCopyright", "1"),
        ])
        .header("User-Agent", UA)
        .send()
        .await
        .map_err(|e| format!("咪咕搜索失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("咪咕搜索解析失败: {e}"))?;
    let group = json.get("songResultData").cloned().unwrap_or(Value::Null);
    let total = group.get("totalCount").and_then(Value::as_u64).unwrap_or(0);
    let mut items = Vec::new();
    if let Some(list) = group.get("resultList").and_then(Value::as_array) {
        for item in list {
            let id = item
                .get("copyrightId")
                .or_else(|| item.get("contentId"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if id.is_empty() {
                continue;
            }
            items.push(MusicInfo {
                id,
                name: item.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
                alia: None,
                ar: vec![Artist {
                    id: String::new(),
                    name: item
                        .get("singerList")
                        .and_then(Value::as_array)
                        .and_then(|a| a.first())
                        .and_then(|s| s.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    alia: None,
                }],
                al: Album {
                    id: String::new(),
                    name: item.get("albumName").and_then(Value::as_str).unwrap_or_default().to_string(),
                    alia: None,
                    covr: item.get("img3").or_else(|| item.get("img1")).and_then(Value::as_str).unwrap_or_default().to_string(),
                },
                dt: 0,
                mv: String::new(),
                fee: 0,
                file: vec![],
            });
        }
    }
    Ok(SearchPage { total, items })
}
