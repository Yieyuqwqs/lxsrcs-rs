//! 酷我（`kw`）。`mobi.s` 的 `convert_url_with_sign` 零 cookie 就能拿 320k；
//! 低码率时服务端回 `bitrate=1`（哨兵值），按 `format/bitrate` 反推实际音质，绝不假装拿到 flac。

use serde_json::Value;

use crate::{
    cache::now_secs,
    itype::{Album, Artist, FileItem, Link, LyricInfo, MusicInfo, SearchPage},
    state::AppState,
};

const ENDPOINT: &str = "https://mobi.kuwo.cn/mobi.s";
const SOURCE: &str = "kwplayer_ar_9.3.0.1_B_jiagu";
const TTL: u64 = 60 * 60;
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36";

fn br_for(qual: &str) -> &'static str {
    match qual {
        "flac" | "flac24bit" => "flac",
        "320k" => "320k",
        "192k" => "192k",
        _ => "128k",
    }
}

async fn mobi(st: &AppState, query: &[(&str, &str)]) -> Result<String, String> {
    let resp = st
        .http
        .get(ENDPOINT)
        .query(query)
        .header("User-Agent", UA)
        .header("Referer", "https://www.kuwo.cn/")
        .send()
        .await
        .map_err(|e| format!("酷我请求失败: {e}"))?;
    resp.text().await.map_err(|e| format!("酷我响应读取失败: {e}"))
}

pub async fn url(st: &AppState, id: &str, qual: &str) -> Result<Link, String> {
    let text = mobi(
        st,
        &[
            ("f", "web"),
            ("source", SOURCE),
            ("type", "convert_url_with_sign"),
            ("br", br_for(qual)),
            ("rid", id),
        ],
    )
    .await?;
    let json: Value = serde_json::from_str(&text).map_err(|_| format!("酷我返回非 JSON: {}", &text[..text.len().min(120)]))?;
    let data = json.get("data").ok_or_else(|| "酷我未返回 data".to_string())?;
    let link = data.get("url").and_then(Value::as_str).unwrap_or_default();
    if link.is_empty() {
        return Err(format!("酷我没有下发链接: {}", &text[..text.len().min(120)]));
    }
    let format = data.get("format").and_then(Value::as_str).unwrap_or("mp3");
    let bitrate = data.get("bitrate").and_then(Value::as_u64).unwrap_or(0);
    let actual = match (format, bitrate) {
        ("flac", b) if b >= 900 => "flac",
        ("flac", _) => "flac",
        (_, b) if b >= 300 => "320k",
        (_, b) if b >= 190 => "192k",
        _ => "128k",
    };
    Ok(Link {
        url: link.to_string(),
        qual: actual.to_string(),
        expi: now_secs() + TTL,
        size: data.get("size").and_then(Value::as_u64),
    })
}

pub async fn lyric(st: &AppState, id: &str) -> Result<LyricInfo, String> {
    // 网页版歌词接口：lrclist 里已经是分好行的时间轴
    let resp = st
        .http
        .get(format!("http://m.kuwo.cn/newh5/singles/songinfoandlrc?musicId={id}"))
        .header("User-Agent", UA)
        .send()
        .await
        .map_err(|e| format!("酷我歌词请求失败: {e}"))?;
    let json: Value = resp.json().await.map_err(|e| format!("酷我歌词解析失败: {e}"))?;
    let list = json
        .get("data")
        .and_then(|d| d.get("lrclist"))
        .and_then(Value::as_array)
        .ok_or_else(|| "酷我没有歌词".to_string())?;
    let mut lyric = String::new();
    for line in list {
        let time = line.get("time").and_then(Value::as_str).unwrap_or_default();
        let text = line.get("lineLyric").and_then(Value::as_str).unwrap_or_default();
        let (Ok(seconds), _) = (time.parse::<f64>(), ()) else { continue };
        let total = (seconds * 100.0).round() as u64;
        let m = total / 6000;
        let s = (total % 6000) / 100;
        let cs = total % 100;
        lyric.push_str(&format!("[{m:02}:{s:02}.{cs:02}]{text}\n"));
    }
    if lyric.is_empty() {
        return Err("酷我没有歌词".to_string());
    }
    Ok(LyricInfo { lyric, ..Default::default() })
}

fn size_param(size: &str) -> u32 {
    match size {
        "s" | "small" => 256,
        "m" | "middle" => 500,
        "l" | "large" | "" => 1024,
        other => other.parse::<u32>().unwrap_or(1024),
    }
}

pub async fn pic(st: &AppState, id: &str, size: &str) -> Result<String, String> {
    let px = size_param(size);
    let resp = st
        .http
        .get("http://artistpicserver.kuwo.cn/pic.web")
        .query(&[("corp", "kuwo"), ("type", "rid_pic"), ("pictype", "500"), ("size", "500"), ("rid", id)])
        .header("User-Agent", UA)
        .send()
        .await
        .map_err(|e| format!("酷我封面请求失败: {e}"))?;
    let text = resp.text().await.unwrap_or_default();
    if !text.trim().starts_with("http") {
        return Err("酷我没有封面".to_string());
    }
    let _ = px;
    Ok(text.trim().to_string())
}

pub async fn info(st: &AppState, id: &str) -> Result<MusicInfo, String> {
    let resp = st
        .http
        .get(format!("http://m.kuwo.cn/newh5/singles/songinfoandlrc?musicId={id}"))
        .header("User-Agent", UA)
        .send()
        .await
        .map_err(|e| format!("酷我信息请求失败: {e}"))?;
    let json: Value = resp.json().await.map_err(|e| format!("酷我信息解析失败: {e}"))?;
    let song = json.get("data").and_then(|d| d.get("songinfo")).ok_or_else(|| "酷我没有歌曲信息".to_string())?;
    let name = song.get("songName").and_then(Value::as_str).unwrap_or_default().to_string();
    let artist = song.get("artist").and_then(Value::as_str).unwrap_or_default().to_string();
    let album = song.get("album").and_then(Value::as_str).unwrap_or_default().to_string();
    let album_id = song.get("albumId").and_then(|v| v.as_i64()).unwrap_or(0).to_string();
    let duration = song.get("songTimeMinutes").and_then(Value::as_str).unwrap_or("0:0");
    let dt = duration
        .split(':')
        .filter_map(|p| p.trim().parse::<u32>().ok())
        .fold(0u32, |acc, part| acc * 60 + part);
    let pic = song.get("pic").and_then(Value::as_str).unwrap_or_default().to_string();
    Ok(MusicInfo {
        id: id.to_string(),
        name,
        alia: None,
        ar: vec![Artist { id: String::new(), name: artist, alia: None }],
        al: Album { id: album_id, name: album, alia: None, covr: pic },
        dt,
        mv: String::new(),
        fee: 0,
        file: vec![],
    })
}

pub async fn search(st: &AppState, keyword: &str, page: u32, size: u32) -> Result<SearchPage, String> {
    let pn = page.saturating_sub(1).to_string();
    let rn = size.clamp(1, 100).to_string();
    let resp = st
        .http
        .get("http://search.kuwo.cn/r.s")
        .query(&[
            ("client", "kt"),
            ("all", keyword),
            ("pn", pn.as_str()),
            ("rn", rn.as_str()),
            ("uid", "0"),
            ("ver", "kwplayer_ar_9.3.0.1"),
            ("vipver", "1"),
            ("show_copyright_off", "1"),
            ("newver", "1"),
            ("ft", "music"),
            ("cluster", "0"),
            ("strategy", "2012"),
            ("encoding", "utf8"),
            ("rformat", "json"),
            ("vermerge", "1"),
            ("mobi", "1"),
        ])
        .header("User-Agent", UA)
        .header("Referer", "https://www.kuwo.cn/")
        .send()
        .await
        .map_err(|e| format!("酷我搜索失败: {e}"))?;
    let text = resp.text().await.unwrap_or_default();
    // 老接口回的是单引号 JSON，先修成合法 JSON
    let fixed = text.replace('\'', "\"");
    let json: Value = serde_json::from_str(&fixed).map_err(|_| "酷我搜索返回无法解析".to_string())?;
    let total = json.get("TOTAL").and_then(Value::as_str).and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut items = Vec::new();
    if let Some(list) = json.get("abslist").and_then(Value::as_array) {
        for item in list {
            let id = item.get("DC_TARGETID").and_then(Value::as_str).unwrap_or_default().to_string();
            if id.is_empty() {
                continue;
            }
            let duration = item.get("DURATION").and_then(Value::as_str).unwrap_or("0");
            let dt = duration.split(':').filter_map(|p| p.trim().parse::<u32>().ok()).fold(0u32, |acc, p| acc * 60 + p);
            items.push(MusicInfo {
                id,
                name: item.get("SONGNAME").and_then(Value::as_str).unwrap_or_default().to_string(),
                alia: None,
                ar: vec![Artist {
                    id: item.get("ARTISTID").and_then(Value::as_str).unwrap_or_default().to_string(),
                    name: item.get("ARTIST").and_then(Value::as_str).unwrap_or_default().to_string(),
                    alia: None,
                }],
                al: Album {
                    id: item.get("ALBUMID").and_then(Value::as_str).unwrap_or_default().to_string(),
                    name: item.get("ALBUM").and_then(Value::as_str).unwrap_or_default().to_string(),
                    alia: None,
                    covr: String::new(),
                },
                dt,
                mv: String::new(),
                fee: 0,
                file: vec![FileItem { qual: "128k".into(), size: None, sizs: None, hash: None }],
            });
        }
    }
    Ok(SearchPage { total, items })
}
