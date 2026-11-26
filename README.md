# lxsrcs-rs

洛雪音乐（LX Music）自定义源服务端的 **Rust 实现**，兼容 Go 版 `lxsrcs`（LX-Source v1.1.17）的
`/api/v0/*` 线格式——客户端导入 `http://<host>:1101/api/v0/lx-script.js` 即可用，脚本无需改动。

## 已实现

| 能力 | 状态 |
| --- | --- |
| 根信息 `/`、内置脚本 `/api/v0/script`（`?raw`/`?key`） | ✅ |
| `/api/v0/{url\|link\|lrc\|lyric\|pic\|mid}` + `?jmp` 302 | ✅ |
| `wy` 取链/歌词/封面/信息/搜索 | ✅ 实测（320k 匿名可取，逐字歌词转 lxlyric） |
| `kw` 取链/歌词/封面/信息/搜索 | ✅ 取链与搜索实测；歌词/封面为公开接口实现 |
| `kg` 取链（v5 签名）/歌词（krc 解码）/信息/搜索 | ✅ 算法与端点已对齐，待真机回归 |
| `mg` 取链（listenSong 302）/歌词/信息/搜索 | ✅ 匿名 128k |
| `tx` 取链（vkey）/歌词/封面/信息/搜索 | ✅ |
| `bi` 取链/封面/信息 | ✅（搜索需 wbi 签名，未做） |
| `lx` 自定义节点转发 | ✅ |
| 接口密钥（`X-Request-Key`/`X-LxM-Auth`/`?key`）、IP 黑白名单、限速 | ✅ |
| 内存分片缓存 + 本地文件缓存（`expi` 语义） | ✅ |
| 错误音频 `/api/v1/file/er/error.mp3?sign=` | ✅（1 秒静音 mp3，签名可配） |
| `/api/v1/search`（wy/kw/kg/mg/tx） | ✅ |
| `mov/vid`（视频）、`/api/v1/plist`、WebUI 静态页 | ❌ 明确返回不支持 |

## 与 Go 版的差异（诚实清单）

1. **WebUI 静态页不做**：`/` 返回接口元数据 JSON，`/status` 返回机器可读状态。
2. **Bn 兜底源（GDStudio/Toubiec/Vkeys）未接入**：它们是第三方私有协议，取不到就不再假装能取。
3. **`mid` 的文件大小**：`wy` 用 `song/detail` 的 `h/m/l/sq/hr`；`kw`/`kg` 部分平台只给 `sizs` 或不给。
4. **视频接口**（`mov`/`vid`）未实现，返回 `code=6`。
5. **`kg` 依赖公开接口**，会员曲目取不到无损时如实降档，不伪造音质。

## 编译与运行

```bash
cargo build --release
./target/release/lxsrcs conf.toml
# 监听 127.0.0.1:1101
```

Android/Termux（aarch64）：

```bash
rustup target add aarch64-linux-android
export ANDROID_NDK_HOME=$HOME/android-ndk   # NDK r26+
cargo build --release --target aarch64-linux-android
```

## 配置

直接复用 Go 版的 `conf.toml`（字段名一致，见仓库根目录示例）。只列关键项：

```toml
[Main]
Listen = '127.0.0.1:1101'
Debug = false

[Srcs.Wy]
Enable = true      # wy/mg/kw 默认可匿名使用

[Auth.ApiPass]
Enable = false
Value = ''         # 开了就必须带 X-Request-Key / ?key
```

## 设计取舍

- **不静默降级**：上游降档时 `ext.qual` 如实返回实际音质（`320k` 而不是谎报 `flac`）。
- **错误可解释**：失败统一 `code=2` + 原因 + 错误音频；参数错误 `code=6`。
- **缓存只存原链**：代理改写留在出口，避免把死链缓存进去（这条是上一代网关踩过的坑）。
