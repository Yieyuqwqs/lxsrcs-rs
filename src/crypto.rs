//! 上游加密：网易云 weapi / eapi、以及配套哈希。
//!
//! 全部手写 AES，不依赖高层封装——只有 CBC/ECB + PKCS7 两种模式，行为可验证，
//! 也避免不同 crate 版本之间的 trait 差异（本机无 rustc，编译风险要压到最低）。

use aes::cipher::{generic_array::GenericArray, BlockEncrypt, KeyInit};
use aes::Aes128;
use num_bigint::BigUint;
use num_traits::Num;

/// 网易云 weapi 公钥模数（从脚本内置的 PEM 里提取，十六进制大写）。
const WY_MODULUS: &str = "E0B509F6259DF8642DBC35662901477DF22677EC152B5FF68ACE615BB7B725152B3AB17A876AEA8A5AA76D2E417629EC4EE341F56135FCCF695280104E0312ECBDA92557C93870114AF6C9D05C4F7F0C3685B7A46BEE255932575CCE10B424D813CFE4875D3E82047B97DDEF52741D546B8E289DC6935B3ECE0462DB0A22B8E7";
const WY_EXPONENT: u32 = 65537;

const WY_PRESET_KEY: &[u8; 16] = b"0CoJUm6Qyw8W8jud";
const WY_IV: &[u8; 16] = b"0102030405060708";
const WY_EAPI_KEY: &[u8; 16] = b"e82ckenh8dichen8";

/// AES-128-ECB + PKCS7。
pub fn aes_ecb_pkcs7(key: &[u8; 16], data: &[u8]) -> Vec<u8> {
    let cipher = Aes128::new(GenericArray::from_slice(key));
    let mut buffer = data.to_vec();
    let pad = 16 - (buffer.len() % 16);
    buffer.extend(std::iter::repeat(pad as u8).take(pad));
    let mut out = Vec::with_capacity(buffer.len());
    for chunk in buffer.chunks(16) {
        let mut block = GenericArray::clone_from_slice(chunk);
        cipher.encrypt_block(&mut block);
        out.extend_from_slice(&block);
    }
    out
}

/// AES-128-CBC + PKCS7。
pub fn aes_cbc_pkcs7(key: &[u8; 16], iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
    let cipher = Aes128::new(GenericArray::from_slice(key));
    let mut buffer = data.to_vec();
    let pad = 16 - (buffer.len() % 16);
    buffer.extend(std::iter::repeat(pad as u8).take(pad));
    let mut prev = *iv;
    let mut out = Vec::with_capacity(buffer.len());
    for chunk in buffer.chunks(16) {
        let mut block = GenericArray::clone_from_slice(chunk);
        for i in 0..16 {
            block[i] ^= prev[i];
        }
        cipher.encrypt_block(&mut block);
        prev.copy_from_slice(&block);
        out.extend_from_slice(&block);
    }
    out
}

/// 裸 RSA（无填充，左侧补零到 128 字节）——网易云 weapi 的 `encSecKey` 就是这个。
pub fn rsa_raw_no_padding(modulus_hex: &str, exponent: u32, message: &[u8]) -> Vec<u8> {
    let modulus = BigUint::from_str_radix(modulus_hex, 16).expect("模数必须是合法十六进制");
    let mut padded = vec![0u8; 128];
    let start = 128usize.saturating_sub(message.len());
    for (i, byte) in message.iter().enumerate() {
        if start + i < 128 {
            padded[start + i] = *byte;
        }
    }
    let m = BigUint::from_bytes_be(&padded);
    let c = m.modpow(&BigUint::from(exponent), &modulus);
    let bytes = c.to_bytes_be();
    let mut out = vec![0u8; 128usize.saturating_sub(bytes.len())];
    out.extend_from_slice(&bytes);
    out
}

/// 网易云 weapi 表单：`params` + `encSecKey`。
pub fn weapi_form(obj: &serde_json::Value) -> Vec<(String, String)> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let text = obj.to_string();
    let first = aes_cbc_pkcs7(WY_PRESET_KEY, WY_IV, text.as_bytes());
    let first_b64 = STANDARD.encode(first);
    let first2 = aes_cbc_pkcs7(WY_PRESET_KEY, WY_IV, first_b64.as_bytes());
    let first2_b64 = STANDARD.encode(first2);

    let secret: String = (0..16)
        .map(|_| {
            const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
            let idx = rand::random::<usize>() % CHARSET.len();
            CHARSET[idx] as char
        })
        .collect();
    let mut key_bytes = [0u8; 16];
    key_bytes.copy_from_slice(secret.as_bytes());
    let params = STANDARD.encode(aes_cbc_pkcs7(&key_bytes, WY_IV, first2_b64.as_bytes()));

    let reversed: Vec<u8> = secret.bytes().rev().collect();
    let enc_sec_key = hex::encode(rsa_raw_no_padding(WY_MODULUS, WY_EXPONENT, &reversed));

    vec![("params".into(), params), ("encSecKey".into(), enc_sec_key)]
}

/// 网易云 eapi 表单：`params` 为十六进制大写。
pub fn eapi_form(path: &str, obj: &serde_json::Value) -> Vec<(String, String)> {
    let text = obj.to_string();
    let digest = crate::util::md5_hex(&format!("nobody{path}use{text}md5forencrypt"));
    let data = format!("{path}-36cd479b6b5-{text}-36cd479b6b5-{digest}");
    let enc = aes_ecb_pkcs7(WY_EAPI_KEY, data.as_bytes());
    vec![("params".into(), hex::encode(enc).to_uppercase())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aes_matches_known_pkcs7_padding() {
        let out = aes_ecb_pkcs7(b"e82ckenh8dichen8", b"abc");
        assert_eq!(out.len(), 16, "3 字节应补齐到 16");
        // 同一输入必须稳定
        assert_eq!(out, aes_ecb_pkcs7(b"e82ckenh8dichen8", b"abc"));
    }

    #[test]
    fn rsa_output_is_128_bytes() {
        let out = rsa_raw_no_padding(WY_MODULUS, WY_EXPONENT, b"secretkey");
        assert_eq!(out.len(), 128);
    }
}
