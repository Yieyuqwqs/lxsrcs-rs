//! 歌词处理：krc（酷狗/酷我逐字）解码 + lrc 规整 + 逐字标记拆分。

use crate::itype::LyricInfo;

/// KRC 解压：magic `krc1` → 异或 → zlib inflate。
pub fn krc_to_lrc(raw: &[u8]) -> String {
    let mut data = raw.to_vec();
    if data.len() > 4 && &data[..4] == b"krc1" {
        data.drain(..4);
    }
    const KEY: [u8; 16] = [0x40, 0x47, 0x61, 0x77, 0x5e, 0x32, 0x74, 0x47, 0x51, 0x36, 0x31, 0x2d, 0xce, 0xd2, 0x6e, 0x69];
    for (i, byte) in data.iter_mut().enumerate() {
        *byte ^= KEY[i % KEY.len()];
    }
    use std::io::Read;
    let mut out = String::new();
    let mut decoder = flate2::read::ZlibDecoder::new(&data[..]);
    if decoder.read_to_string(&mut out).is_ok() && !out.is_empty() {
        return krc_text_to_lrc(&out);
    }
    // 不是压缩体（服务端偶尔直接给 lrc 文本）
    String::from_utf8_lossy(raw).to_string()
}

/// KRC 文本 `[start,dur]字<start,dur,0>字` → 标准 lrc（保留逐字信息到 lxlyric）。
fn krc_text_to_lrc(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with('[') {
            continue;
        }
        let Some(close) = line.find(']') else { continue };
        let head = &line[1..close];
        let Some((start, _dur)) = head.split_once(',') else { continue };
        let Ok(start_ms) = start.parse::<u64>() else { continue };
        let words = &line[close + 1..];
        let plain = strip_word_tags(words);
        if plain.trim().is_empty() {
            continue;
        }
        out.push_str(&format!("{}{}\n", fmt_tag(start_ms), plain));
    }
    out
}

fn strip_word_tags(input: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for ch in input.chars() {
        match ch {
            '<' => depth += 1,
            '>' if depth > 0 => depth -= 1,
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

fn fmt_tag(ms: u64) -> String {
    let m = ms / 60000;
    let s = (ms % 60000) / 1000;
    let cs = (ms % 1000) / 10;
    format!("[{m:02}:{s:02}.{cs:02}]")
}

/// 规整时间标签：`[00:12:34]` → `[00:12.34]`，兼容少见的两位毫秒。
pub fn normalize_lrc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let mut line = line.trim_end().to_string();
        if line.starts_with('[') {
            if let Some(close) = line.find(']') {
                let head = line[1..close].to_string();
                let parts: Vec<&str> = head.split(':').collect();
                if parts.len() == 3 && parts[0].len() == 2 && parts[1].len() == 2 {
                    let rest = line[close + 1..].to_string();
                    line = format!("[{}:{}.{}]{}", parts[0], parts[1], parts[2], rest);
                }
            }
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// 把一段 lrc 文本拆成「纯歌词 + 逐字歌词」两块。
pub fn split_lrc(text: &str) -> LyricInfo {
    let normalized = normalize_lrc(text);
    let mut lyric = String::new();
    let mut lxlyric = String::new();
    for line in normalized.lines() {
        if !line.starts_with('[') {
            continue;
        }
        let plain = strip_word_tags(line);
        lyric.push_str(&plain);
        lyric.push('\n');
        if line.contains('<') {
            lxlyric.push_str(line);
            lxlyric.push('\n');
        } else {
            lxlyric.push_str(&plain);
            lxlyric.push('\n');
        }
    }
    LyricInfo { lyric, lxlyric, ..Default::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_three_part_timestamps() {
        assert_eq!(normalize_lrc("[00:12:34]hello\n"), "[00:12.34]hello\n");
    }

    #[test]
    fn strips_word_tags_for_plain_lyric() {
        let info = split_lrc("[00:01.00]<0,200>你<200,300>好\n");
        assert!(info.lyric.contains("你好"));
        assert!(info.lxlyric.contains('<'));
    }
}
