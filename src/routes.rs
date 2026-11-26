//! 路由装配：根信息、内置脚本、`/api/v0/*`、`/api/v1/search|file`、状态页。

use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    middleware,
    routing::get,
    Json, Router,
};

use crate::{auth, cache::now_secs, itype::Link, resp::{self, Resp}, sources, state::AppState, util};

type St = State<Arc<AppState>>;
type Params = Query<HashMap<String, String>>;

pub fn build(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/status", get(status))
        .route("/status/", get(status))
        .route("/api/v0/script", get(script))
        .route("/api/v0/lx-script.js", get(script))
        .route("/api/v0/{*path}", get(v0))
        .route("/api/v1/search", get(search))
        .route("/api/v1/file/{kind}/{name}", get(error_file))
        .fallback(not_found)
        .layer(middleware::from_fn(crate::access::access_log))
        .with_state(state)
}

fn host_of(headers: &HeaderMap) -> String {
    if let Some(host) = headers.get("host").and_then(|v| v.to_str().ok()) {
        let proto = headers.get("x-forwarded-proto").and_then(|v| v.to_str().ok()).unwrap_or("http");
        return format!("{proto}://{host}");
    }
    "http://127.0.0.1".to_string()
}

async fn root(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if let Some(resp) = precheck(&st, &headers, &HashMap::new()) {
        return resp;
    }
    let mut source = serde_json::Map::new();
    for plat in sources::PLATFORMS {
        match sources::qualities(&st.cfg, plat) {
            Some(list) => source.insert(plat.to_string(), serde_json::json!(list)),
            None => source.insert(plat.to_string(), serde_json::Value::Null),
        };
    }
    let body = serde_json::json!({
        "version": st.version,
        "name": "lx-music-source",
        "msg": "Hello~::^-^::~v1~",
        "source": source,
        "script": {
            "ver": "1.0.1",
            "log": "Rust 实现（LX-Source 兼容）",
            "url": "lx-script.js",
            "force": false
        },
        "summary": st.stats.summary(),
        "auth": { "apikey": st.cfg.auth.api_pass.enable || st.cfg.auth.uni_pass.enable },
    });
    Json(body).into_response()
}

async fn script(State(st): State<Arc<AppState>>, headers: HeaderMap, params: Params) -> Response {
    let query = params.0;
    if let Some(resp) = precheck(&st, &headers, &query) {
        return resp;
    }
    let host = host_of(&headers);
    let key = auth::key_from_request(&headers, &query).unwrap_or_default();
    let body = util::render_script(&st.cfg, &st.script, &format!("{host}/api/v0/"), &key);
    if query.contains_key("raw") {
        let name = host.trim_start_matches("http://").trim_start_matches("https://");
        (
            StatusCode::OK,
            [
                ("Content-Type", "application/octet-stream".to_string()),
                ("Content-Disposition", format!("attachment; filename=\"lx-script_{name}.js\"")),
            ],
            body,
        )
            .into_response()
    } else {
        ([(axum::http::header::CONTENT_TYPE, "text/javascript; charset=utf-8".to_string())], body).into_response()
    }
}

async fn v0(
    State(st): State<Arc<AppState>>,
    Path(path): Path<String>,
    Query(query): Params,
    headers: HeaderMap,
) -> Response {
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    if segments.len() < 3 {
        return Resp::bad("参数不全").into_response();
    }
    let (method, plat, id) = (segments[0].to_string(), segments[1].to_string(), segments[2].to_string());
    let qual = segments.get(3).map(|s| s.to_string()).unwrap_or_default();
    
    if let Some(resp) = precheck(&st, &headers, &query) {
        return resp;
    }
    if !sources::verify_id(&plat, &id) {
        return Resp::bad(sources::E_ID).into_response();
    }
    st.stats.bump_access();
    let host = host_of(&headers);
    let jmp = query.contains_key("jmp");

    match method.as_str() {
        "url" | "link" => {
            let quality = if qual.is_empty() { "128k".to_string() } else { qual };
            match resolve_link(&st, &plat, &id, &quality, &host).await {
                Ok(resp) => {
                    if jmp && resp.code == 0 {
                        if let Some(url) = resp.data.as_str() {
                            return Redirect::temporary(url).into_response();
                        }
                    }
                    resp.into_response()
                }
                Err(msg) => fail_resp(&st, &host, msg).into_response(),
            }
        }
        "lrc" | "lyric" => match sources::resolve_lyric(&st, &plat, &id).await {
            Ok(info) => Resp::ok(serde_json::json!(info)).into_response(),
            Err(msg) => fail_resp(&st, &host, msg).into_response(),
        },
        "pic" | "cover" => {
            let size = if qual.is_empty() { "l".to_string() } else { qual };
            match sources::resolve_pic(&st, &plat, &id, &size).await {
                Ok(url) => {
                    if jmp {
                        return Redirect::temporary(&url).into_response();
                    }
                    Resp::ok(serde_json::json!(url)).into_response()
                }
                Err(msg) => fail_resp(&st, &host, msg).into_response(),
            }
        }
        "mid" => match sources::resolve_info(&st, &plat, &id).await {
            Ok(info) => Resp::ok(serde_json::json!(info)).into_response(),
            Err(msg) => fail_resp(&st, &host, msg).into_response(),
        },
        "mov" | "mv" | "vid" => fail_resp(&st, &host, "视频接口暂未实现".to_string()).into_response(),
        other => Resp::bad(format!("无效源方法:'{other}'")).into_response(),
    }
}

async fn resolve_link(st: &Arc<AppState>, plat: &str, id: &str, qual: &str, host: &str) -> Result<Resp, String> {
    let key = format!("url/{plat}/{id}/{qual}");
    if let Some(hit) = st.mem.get(&key) {
        return Ok(Resp::ok(serde_json::json!(hit)).with_msg("Memory HIT"));
    }
    if let Some(hit) = st.file.get(&key) {
        st.mem.set(&key, &hit, 600);
        return Ok(Resp::ok(serde_json::json!(hit)).with_msg("Cache HIT"));
    }
    st.stats.bump_request(plat);
    match sources::resolve_url(st, plat, id, qual).await {
        Ok(link) => {
            st.stats.bump_success(plat);
            let ttl = sources::cache_ttl(plat);
            st.mem.set(&key, &link.url, ttl);
            if st.cfg.data.type_ == 0 {
                st.file.set(&key, &link.url, now_secs() + st.cfg.data.local.expi);
            }
            Ok(Resp::ok(serde_json::json!(link.url)).with_ext(serde_json::json!({ "qual": link.qual, "expi": link.expi })))
        }
        Err(msg) => Err(msg),
    }
    .map_err(|msg| {
        let _ = host;
        msg
    })
}

fn fail_resp(st: &Arc<AppState>, host: &str, msg: String) -> Resp {
    let audio = st.err_audio(host);
    Resp::fail(msg).with_ext(serde_json::json!({ "qual": "temp", "expi": now_secs() + st.cfg.errs.expi })).fallback_audio(&audio)
}

async fn search(State(st): State<Arc<AppState>>, Query(query): Params, headers: HeaderMap) -> Response {
    if let Some(resp) = precheck(&st, &headers, &query) {
        return resp;
    }
    let plat = query.get("pt").cloned().unwrap_or_else(|| "wy".to_string());
    let keyword = query.get("q").cloned().unwrap_or_default();
    if keyword.is_empty() {
        return Resp::bad("缺少查询内容 q").into_response();
    }
    let page: u32 = query.get("pg").and_then(|v| v.parse().ok()).unwrap_or(1);
    let size: u32 = query.get("sz").and_then(|v| v.parse().ok()).unwrap_or(10);
    match sources::resolve_search(&st, &plat, &keyword, page, size).await {
        Ok(page) => Resp::ok(serde_json::json!(page)).into_response(),
        Err(msg) => Resp::fail(msg).into_response(),
    }
}

async fn error_file(State(st): State<Arc<AppState>>, Path((kind, _name)): Path<(String, String)>, Query(query): Params) -> Response {
    if kind == "er" {
        let sign = query.get("sign").map(String::as_str).unwrap_or_default();
        if !sign.is_empty() && !util::error_sign_ok(&st.cfg, sign) {
            return StatusCode::GONE.into_response();
        }
        return ([(axum::http::header::CONTENT_TYPE, "audio/mpeg")], st.silence_mp3.clone()).into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}

async fn status(State(st): State<Arc<AppState>>) -> Response {
    Json(serde_json::json!({
        "name": "lxsrcs-rs",
        "version": st.version,
        "go_compatible": "1.1.17",
        "uptime": util::fmt_duration(st.start.elapsed().as_secs()),
        "summary": st.stats.summary(),
        "sources": sources::PLATFORMS.iter().map(|p| (*p, sources::qualities(&st.cfg, p).is_some())).collect::<HashMap<_, _>>(),
    }))
    .into_response()
}

async fn not_found() -> Response {
    Resp::err(resp::CODE_PARAM, "接口不存在").into_response()
}

/// 统一前置检查：密钥 → IP 黑白名单 → 限流。
fn precheck(st: &Arc<AppState>, headers: &HeaderMap, query: &HashMap<String, String>) -> Option<Response> {
    let ip = st.client_ip(headers, None);
    if !auth::ip_allowed(&st.cfg, ip) {
        return Some(Resp::banned("IP 已被封禁").into_response());
    }
    if !st.limiter.allow(&st.cfg, ip) {
        return Some(Resp::limit("请求过于频繁").into_response());
    }
    let key = auth::key_from_request(headers, query);
    if !auth::verify_key(&st.cfg, key.as_deref()) {
        return Some(Resp::auth("密钥验证失败").into_response());
    }
    None
}

/// 让 `Link` 进日志时更可读（`#[allow]` 是因为部分字段只在签名里用到）。
#[allow(dead_code)]
fn log_link(plat: &str, id: &str, link: &Link) {
    tracing::info!(plat, id, qual = %link.qual, "解析成功");
}
