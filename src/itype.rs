//! 对外数据模型，字段名与 Go 版 `IMusicInfo / ILyricInfo / ILinkInfo` 一一对应。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Artist {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alia: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Album {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alia: Option<Vec<String>>,
    #[serde(default)]
    pub covr: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileItem {
    pub qual: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sizs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MusicInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub alia: Option<Vec<String>>,
    #[serde(default)]
    pub ar: Vec<Artist>,
    #[serde(default)]
    pub al: Album,
    #[serde(default)]
    pub dt: u32,
    #[serde(default)]
    pub mv: String,
    #[serde(default)]
    pub fee: u32,
    #[serde(default)]
    pub file: Vec<FileItem>,
}

impl MusicInfo {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self { id: id.into(), name: name.into(), ..Default::default() }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LyricInfo {
    #[serde(default)]
    pub lyric: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tlyric: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub rlyric: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub lxlyric: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LinkInfo {
    pub link: String,
    #[serde(default)]
    pub qual: String,
    #[serde(default)]
    pub expi: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MovieInfo {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub desc: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub covr: String,
    #[serde(default)]
    pub file: Vec<FileItem>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchPage {
    pub total: u64,
    pub items: Vec<MusicInfo>,
}

/// 一条可播放直链 + 实际音质 + 过期时间。
#[derive(Debug, Clone)]
pub struct Link {
    pub url: String,
    pub qual: String,
    pub expi: u64,
    pub size: Option<u64>,
}

impl Link {
    pub fn new(url: impl Into<String>, qual: impl Into<String>, expi: u64) -> Self {
        Self { url: url.into(), qual: qual.into(), expi, size: None }
    }
}
