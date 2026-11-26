//! 酷狗（`kg`）。v5 `trackercdn` 签名取链；歌词走 `lyrics.kugou.com`（krc/逐字）。

use serde_json::Value;

use crate::{
    cache::now_secs,
    itype::{Album, Artist, FileItem, Link, LyricInfo, MusicInfo, SearchPage},
    state::AppState,
};

const ENDPOINT: &str = "https://trackercdn.kugou.com/v5/url";
const SIGN_SALT: &str = "OIlwieks28dk2k092lksi2UIkp";
const KEY_SUFFIX: &str = "57ae12eb6890223e355ccfcb74edf70d1005-0";
const UA: &str = "Android712-AndroidPhone-12227-130-0-play-0";
const TTL: u64 = 24 * 60 * 60;

fn quality_arg(qual: &str) -> &'static str {
    match qual {
        "flac" | "flac24bit" => "flac",
        "320k" => "320",
        _ => "128",
    }
}

fn signed_query(hash: &str, qual: &str) -> String {
    let key = crate::util::md5_hex(&format!("{hash}{KEY_SUFFIX}"));
    let clienttime = now_secs().to_string();
    let mut params: Vec<(&str, String)> = vec![
        ("userid", "0".into()),
        ("area_code", "1".into()),
        ("module", String::new()),
        ("hash", hash.to_string()),
        ("appid", "1005".into()),
        ("ssa_flag", "is_fromtrack".into()),
        ("version", "12029".into()),
        ("vipType", "6".into()),
        ("ptype", "0".into()),
        ("token", String::new()),
        ("mtype", "3".into()),
        ("quality", quality_arg(qual).into()),
        ("album_audio_id", "0".into()),
        ("behavior", "play".into()),
        ("pid", "2".into()),
        ("clienttime", clienttime),
        ("cmd", "26".into()),
        ("uuid", "-".into()),
        ("mid", "-".into()),
        ("dfid", "-".into()),
        ("clientver", "12029".into()),
        ("pidversion", "3001".into()),
        ("key", key),
    ];
    params.sort_by(|a, b| a.0.cmp(b.0));
    let joined: String = params.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("");
    let signature = crate::util::md5_hex(&format!("{SIGN_SALT}{joined}{SIGN_SALT}"));
    let mut query = params.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&");
    query.push_str("&signature=");
    query.push_str(&signature);
    query
}

pub async fn url(st: &AppState, id: &str, qual: &str) -> Result<Link, String> {
    let hash = id.to_uppercase();
    let resp = st
        .http
        .get(format!("{ENDPOINT}?{}", signed_query(&hash, qual)))
        .header("User-Agent", UA)
        .send()
        .await
        .map_err(|e| format!("酷狗请求失败: {e}"))?;
    let json: Value = resp.json().await.map_err(|e| format!("酷狗解析失败: {e}"))?;
    let status = json.get("status").and_then(Value::as_i64).unwrap_or(-1);
    let link = json
        .get("url")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(Value::as_str)
        .unwrap_or_default();
    if status != 1 || link.is_empty() {
        return Err(format!("酷狗未下发链接: status={status}"));
    }
    let bitrate = json.get("bitRate").and_then(Value::as_u64).unwrap_or(0);
    let actual = match bitrate {
        b if b >= 900 => "flac",
        b if b >= 300 => "320k",
        b if b >= 190 => "192k",
        _ => "128k",
    };
    Ok(Link { url: link.to_string(), qual: actual.to_string(), expi: now_secs() + TTL, size: json.get("fileSize").and_then(Value::as_u64) })
}

pub async fn lyric(st: &AppState, id: &str) -> Result<LyricInfo, String> {
    let search: Value = st
        .http
        .get("https://lyrics.kugou.com/search")
        .query(&[("ver", "1"), ("man", "yes"), ("client", "android"), ("keyword", ""), ("duration", "240000"), ("hash", id)])
        .header("User-Agent", UA)
        .send()
        .await
        .map_err(|e| format!("酷狗歌词搜索失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("酷狗歌词搜索解析失败: {e}"))?;
    let candidate = search
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .ok_or_else(|| "酷狗没有歌词".to_string())?;
    let cid = candidate.get("id").and_then(Value::as_str).unwrap_or_default();
    let accesskey = candidate.get("accesskey").and_then(Value::as_str).unwrap_or_default();
    let download: Value = st
        .http
        .get("https://lyrics.kugou.com/download")
        .query(&[("ver", "1"), ("client", "pc"), ("id", cid), ("accesskey", accesskey), ("fmt", "krc"), ("charset", "utf8")])
        .header("User-Agent", UA)
        .send()
        .await
        .map_err(|e| format!("酷狗歌词下载失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("酷狗歌词下载解析失败: {e}"))?;
    let content = download.get("content").and_then(Value::as_str).unwrap_or_default();
    if content.is_empty() {
        return Err("酷狗歌词为空".to_string());
    }
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let raw = STANDARD.decode(content).map_err(|e| format!("酷狗歌词 base64 解码失败: {e}"))?;
    let text = crate::lyric::krc_to_lrc(&raw);
    if text.trim().is_empty() {
        return Err("酷狗歌词解码为空".to_string());
    }
    Ok(crate::lyric::split_lrc(&text))
}

pub async fn pic(_st: &AppState, id: &str, _size: &str) -> Result<String, String> {
    // 酷狗图片是 `{size}` 占位模板，这里统一给 500px
    Ok(format!("https://imge.kugou.com/stdmusic/500/{}", id))
}

pub async fn info(st: &AppState, id: &str) -> Result<MusicInfo, String> {
    let json: Value = st
        .http
        .post("http://gateway.kugou.com/v3/album_audio/audio")
        .header("KG-THash", "13a3164")
        .header("KG-RC", "1")
        .header("x-router", "kmr.service.kugou.com")
        .header("User-Agent", UA)
        .json(&serde_json::json!({
            "area_code": "1", "show_privilege": 1, "show_album_info": "1", "is_publish": "",
            "appid": 1005, "clientver": 11451, "mid": "1", "dfid": "-", "clienttime": 0,
            "key": "OIlwieks28dk2k092lksi2UIkp",
            "fields": "album_info,author_name,audio_info,ori_audio_name,base,songname",
            "data": [id.to_uppercase()],
        }))
        .send()
        .await
        .map_err(|e| format!("酷狗信息请求失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("酷狗信息解析失败: {e}"))?;
    let item = json.get("data").and_then(Value::as_array).and_then(|a| a.first()).and_then(|a| a.as_array()).and_then(|a| a.first());
    let item = item.ok_or_else(|| "酷狗没有歌曲信息".to_string())?;
    let name = item.get("songname").and_then(Value::as_str).unwrap_or_default().to_string();
    let author = item.get("author_name").and_then(Value::as_str).unwrap_or_default().to_string();
    let album = item.get("album_info").and_then(Value::as_array).and_then(|a| a.first());
    let album_name = album.and_then(|a| a.get("album_name")).and_then(Value::as_str).unwrap_or_default().to_string();
    let album_id = album.and_then(|a| a.get("album_id")).and_then(Value::as_i64).unwrap_or(0).to_string();
    let img = album.and_then(|a| a.get("cover")).and_then(Value::as_str).unwrap_or_default().to_string();
    let dt = item.get("audio_info").and_then(|a| a.get("duration")).and_then(Value::as_u64).unwrap_or(0) as u32 / 1000;
    Ok(MusicInfo {
        id: id.to_string(),
        name,
        alia: None,
        ar: vec![Artist { id: String::new(), name: author, alia: None }],
        al: Album { id: album_id, name: album_name, alia: None, covr: img },
        dt,
        mv: String::new(),
        fee: 0,
        file: vec![FileItem { qual: "flac".into(), size: None, sizs: None, hash: Some(id.to_uppercase()) }],
    })
}

pub async fn search(st: &AppState, keyword: &str, page: u32, size: u32) -> Result<SearchPage, String> {
    let json: Value = st
        .http
        .get("http://mobilecdn.kugou.com/api/v3/search/song")
        .query(&[("format", "json"), ("keyword", keyword), ("page", &page.to_string()), ("pagesize", &size.to_string()), ("showtype", "1")])
        .header("User-Agent", UA)
        .send()
        .await
        .map_err(|e| format!("酷狗搜索失败: {e}"))?
        .json()
        .await
        .map_err(|e| format!("酷狗搜索解析失败: {e}"))?;
    let data = json.get("data").cloned().unwrap_or(Value::Null);
    let total = data.get("total").and_then(Value::as_u64).unwrap_or(0);
    let mut items = Vec::new();
    if let Some(list) = data.get("info").and_then(Value::as_array) {
        for item in list {
            let hash = item.get("hash").and_then(Value::as_str).unwrap_or_default().to_string();
            if hash.is_empty() {
                continue;
            }
            items.push(MusicInfo {
                id: hash,
                name: item.get("songname").and_then(Value::as_str).unwrap_or_default().to_string(),
                alia: None,
                ar: vec![Artist { id: String::new(), name: item.get("singername").and_then(Value::as_str).unwrap_or_default().to_string(), alia: None }],
                al: Album { id: item.get("album_id").and_then(Value::as_i64).unwrap_or(0).to_string(), name: item.get("album_name").and_then(Value::as_str).unwrap_or_default().to_string(), alia: None, covr: String::new() },
                dt: item.get("duration").and_then(Value::as_u64).unwrap_or(0) as u32,
                mv: String::new(),
                fee: 0,
                file: vec![FileItem { qual: "128k".into(), size: None, sizs: None, hash: None }],
            });
        }
    }
    Ok(SearchPage { total, items })
}
