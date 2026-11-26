//! 小洛源（`lx`）：转发到配置里的自定义节点（`[[Srcs.Lx.Node]]`），取 `orig` 音质。

use crate::{cache::now_secs, itype::Link, state::AppState};

pub async fn url(st: &AppState, id: &str, _qual: &str) -> Result<Link, String> {
    let node = st
        .cfg
        .srcs
        .lx
        .node
        .iter()
        .find(|n| n.enable && !n.addr.is_empty())
        .ok_or_else(|| "没有可用的自定义节点".to_string())?;
    let mut request = st
        .http
        .get(format!("{}/api/v0/url/lx/{id}/orig", node.addr.trim_end_matches('/')))
        .header("User-Agent", if node.ua.is_empty() { crate::util::default_ua() } else { node.ua.clone() });
    if !node.pass.is_empty() {
        request = request.header("X-Request-Key", node.pass.clone());
    }
    let json: serde_json::Value = request.send().await.map_err(|e| format!("自定义节点请求失败: {e}"))?.json().await.map_err(|e| format!("自定义节点解析失败: {e}"))?;
    let data = json.get("data").and_then(|d| d.as_str()).unwrap_or_default();
    if data.is_empty() {
        return Err(format!("自定义节点未下发链接: {}", json.get("msg").and_then(|m| m.as_str()).unwrap_or("")));
    }
    Ok(Link { url: data.to_string(), qual: "orig".to_string(), expi: now_secs() + 12 * 60 * 60, size: None })
}
