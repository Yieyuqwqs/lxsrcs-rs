//! 统一响应：`{code,msg,data,ext?}` + 与 Go 版一致的 HTTP 状态码映射。
//!
//! 返回码对应表（照抄 Go 版 `src/middleware/resp`，脚本按 code 分支）：
//!
//! | code | HTTP | 含义 |
//! | --- | --- | --- |
//! | 0 | 200 | 成功 |
//! | 1 | 403 | IP 被封禁 |
//! | 2 | 503 | 获取失败（data 里给错误音频） |
//! | 3 | 401 | 密钥验证失败 |
//! | 4 | 500 | 服务器内部错误 |
//! | 5 | 429 | 请求过于频繁 |
//! | 6 | 400 | 参数错误 / 不支持的平台 |

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Map, Value};

pub const CODE_OK: i32 = 0;
pub const CODE_BAN: i32 = 1;
pub const CODE_FAIL: i32 = 2;
pub const CODE_AUTH: i32 = 3;
pub const CODE_INNER: i32 = 4;
pub const CODE_LIMIT: i32 = 5;
pub const CODE_PARAM: i32 = 6;

#[derive(Debug, Clone)]
pub struct Resp {
    pub code: i32,
    pub msg: String,
    pub data: Value,
    pub ext: Option<Value>,
}

impl Default for Resp {
    fn default() -> Self {
        Self { code: CODE_OK, msg: String::new(), data: Value::Null, ext: None }
    }
}

impl Resp {
    pub fn ok(data: Value) -> Self {
        Self { code: CODE_OK, msg: String::new(), data, ext: None }
    }

    pub fn ok_ext(data: Value, ext: Value) -> Self {
        Self { code: CODE_OK, msg: String::new(), data, ext: Some(ext) }
    }

    pub fn with_msg(mut self, msg: impl Into<String>) -> Self {
        self.msg = msg.into();
        self
    }

    pub fn with_ext(mut self, ext: Value) -> Self {
        self.ext = Some(ext);
        self
    }

    pub fn err(code: i32, msg: impl Into<String>) -> Self {
        Self { code, msg: msg.into(), data: Value::Null, ext: None }
    }

    pub fn fail(msg: impl Into<String>) -> Self {
        Self::err(CODE_FAIL, msg)
    }

    pub fn bad(msg: impl Into<String>) -> Self {
        Self::err(CODE_PARAM, msg)
    }

    pub fn auth(msg: impl Into<String>) -> Self {
        Self::err(CODE_AUTH, msg)
    }

    pub fn limit(msg: impl Into<String>) -> Self {
        Self::err(CODE_LIMIT, msg)
    }

    pub fn banned(msg: impl Into<String>) -> Self {
        Self::err(CODE_BAN, msg)
    }

    fn status(&self) -> StatusCode {
        match self.code {
            CODE_OK => StatusCode::OK,
            CODE_BAN => StatusCode::FORBIDDEN,
            CODE_FAIL => StatusCode::SERVICE_UNAVAILABLE,
            CODE_AUTH => StatusCode::UNAUTHORIZED,
            CODE_INNER => StatusCode::INTERNAL_SERVER_ERROR,
            CODE_LIMIT => StatusCode::TOO_MANY_REQUESTS,
            CODE_PARAM => StatusCode::BAD_REQUEST,
            _ => StatusCode::OK,
        }
    }

    /// 失败（code=2）且没有 data 时，按 Go 版行为补错误音频链接。
    pub fn fallback_audio(mut self, err_mp3: &str) -> Self {
        if self.code == CODE_FAIL && (self.data.is_null() || self.data.as_str() == Some("")) && !err_mp3.is_empty() {
            self.data = Value::String(err_mp3.to_string());
        }
        self
    }

    pub fn body(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("code".into(), json!(self.code));
        obj.insert("msg".into(), json!(self.msg));
        obj.insert("data".into(), self.data.clone());
        if let Some(ext) = &self.ext {
            obj.insert("ext".into(), ext.clone());
        }
        Value::Object(obj)
    }

    pub fn into_response(self) -> Response {
        let status = self.status();
        (status, Json(self.body())).into_response()
    }
}
