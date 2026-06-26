//! Borrowed from <https://github.com/cloudwego/sonic-rs/blob/v0.5.5/src/util/string.rs>
//!
//! Only takes the string escaping part to avoid the abstraction overhead.

#![allow(clippy::incompatible_msrv)]

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
use std::arch::is_x86_feature_detected;

mod simd;

pub(crate) const QUOTE_TAB: [(u8, [u8; 8]); 256] = [
    // 0x00 ~ 0x1f
    (6, *b"\\u0000\0\0"),
    (6, *b"\\u0001\0\0"),
    (6, *b"\\u0002\0\0"),
    (6, *b"\\u0003\0\0"),
    (6, *b"\\u0004\0\0"),
    (6, *b"\\u0005\0\0"),
    (6, *b"\\u0006\0\0"),
    (6, *b"\\u0007\0\0"),
    (2, *b"\\b\0\0\0\0\0\0"),
    (2, *b"\\t\0\0\0\0\0\0"),
    (2, *b"\\n\0\0\0\0\0\0"),
    (6, *b"\\u000b\0\0"),
    (2, *b"\\f\0\0\0\0\0\0"),
    (2, *b"\\r\0\0\0\0\0\0"),
    (6, *b"\\u000e\0\0"),
    (6, *b"\\u000f\0\0"),
    (6, *b"\\u0010\0\0"),
    (6, *b"\\u0011\0\0"),
    (6, *b"\\u0012\0\0"),
    (6, *b"\\u0013\0\0"),
    (6, *b"\\u0014\0\0"),
    (6, *b"\\u0015\0\0"),
    (6, *b"\\u0016\0\0"),
    (6, *b"\\u0017\0\0"),
    (6, *b"\\u0018\0\0"),
    (6, *b"\\u0019\0\0"),
    (6, *b"\\u001a\0\0"),
    (6, *b"\\u001b\0\0"),
    (6, *b"\\u001c\0\0"),
    (6, *b"\\u001d\0\0"),
    (6, *b"\\u001e\0\0"),
    (6, *b"\\u001f\0\0"),
    // 0x20 ~ 0x2f
    (0, [0; 8]),
    (0, [0; 8]),
    (2, *b"\\\"\0\0\0\0\0\0"),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    // 0x30 ~ 0x3f
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    // 0x40 ~ 0x4f
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    // 0x50 ~ 0x5f
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (2, *b"\\\\\0\0\0\0\0\0"),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    // 0x60 ~ 0xff
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
    (0, [0; 8]),
];

pub(crate) const NEED_ESCAPED: [u8; 256] = [
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[inline(always)]
fn format_string(value: &str, dst: &mut [u8]) -> usize {
    #[cfg(target_arch = "aarch64")]
    {
        let has_neon = cfg!(target_os = "macos") || std::arch::is_aarch64_feature_detected!("neon");
        if has_neon {
            unsafe { simd::neon::format_string(value, dst) }
        } else {
            simd::v128::format_string(value, dst)
        }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(feature = "avx512")]
        {
            if is_x86_feature_detected!("avx512f") {
                return unsafe { simd::avx512::format_string(value, dst) };
            }
        }
        if is_x86_feature_detected!("avx2") {
            unsafe { simd::avx2::format_string(value, dst) }
        } else if is_x86_feature_detected!("sse2") {
            unsafe { simd::sse2::format_string(value, dst) }
        } else {
            simd::v128::format_string(value, dst)
        }
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86", target_arch = "x86_64")))]
    {
        simd::v128::format_string(value, dst)
    }
}

pub fn escape(value: &str) -> String {
    let capacity = value.len() * 6 + 32 + 3;
    let mut buf = Vec::with_capacity(capacity);
    #[allow(clippy::uninit_vec)]
    unsafe {
        buf.set_len(capacity)
    };
    let cnt = format_string(value, &mut buf);
    unsafe { buf.set_len(cnt) };
    unsafe { String::from_utf8_unchecked(buf) }
}

/// # Panics
///
/// Panics if the buffer is not large enough. Allocate enough capacity for dst.
pub fn escape_into<S: AsRef<str>>(value: S, dst: &mut Vec<u8>) {
    let value = value.as_ref();
    let old_len = dst.len();

    // SAFETY: We've reserved enough capacity above, and format_string will
    // write valid UTF-8 bytes. We'll set the correct length after.
    unsafe {
        // Get a slice that includes the spare capacity
        let spare =
            std::slice::from_raw_parts_mut(dst.as_mut_ptr().add(old_len), dst.capacity() - old_len);
        let cnt = format_string(value, spare);
        dst.set_len(old_len + cnt);
    }
}

#[cfg(test)]
mod tests {
    use std::fs::read_dir;
    use std::path::{Path, PathBuf};

    use rand::seq::SliceRandom;

    use super::*;

    #[test]
    fn test_escape_ascii_json_string() {
        let fixture = r#"abcdefghijklmnopqrstuvwxyz .*? hello world escape json string"#;
        assert_eq!(escape(fixture), serde_json::to_string(fixture).unwrap());
    }

    #[test]
    fn test_escape_json_string() {
        let mut fixture = String::new();
        for i in 0u8..=0x1F {
            fixture.push(i as char);
        }
        fixture.push('\t');
        fixture.push('\x08');
        fixture.push('\x09');
        fixture.push('\x0A');
        fixture.push('\x0C');
        fixture.push('\x0D');
        fixture.push('\x22');
        fixture.push('\x5C');
        fixture.push_str("normal string");
        fixture.push('😊');
        fixture.push_str("中文 English 🚀 \n❓ 𝄞");
        escape(fixture.as_str());
        assert_eq!(
            escape(fixture.as_str()),
            serde_json::to_string(fixture.as_str()).unwrap(),
            "fixture: {:?}",
            fixture
        );
    }

    // Test cases for various string sizes to cover different SIMD paths

    #[test]
    fn test_empty_string() {
        assert_eq!(escape(""), r#""""#);
    }

    #[test]
    fn test_very_small_strings() {
        // Less than 16 bytes (SSE register size)
        assert_eq!(escape("a"), r#""a""#);
        assert_eq!(escape("ab"), r#""ab""#);
        assert_eq!(escape("hello"), r#""hello""#);
        assert_eq!(escape("hello\n"), r#""hello\n""#);
        assert_eq!(escape("\""), r#""\"""#);
        assert_eq!(escape("\\"), r#""\\""#);
        assert_eq!(escape("\t"), r#""\t""#);
        assert_eq!(escape("\r\n"), r#""\r\n""#);
    }

    #[test]
    fn test_small_strings_16_bytes() {
        // Exactly 16 bytes - SSE register boundary
        let s16 = "0123456789abcdef";
        assert_eq!(s16.len(), 16);
        assert_eq!(escape(s16), serde_json::to_string(s16).unwrap());

        // 16 bytes with escapes
        let s16_esc = "01234567\t9abcde";
        assert_eq!(s16_esc.len(), 15); // \t is 1 byte
        assert_eq!(escape(s16_esc), serde_json::to_string(s16_esc).unwrap());
    }

    #[test]
    fn test_medium_strings_32_bytes() {
        // Exactly 32 bytes - AVX2 register boundary
        let s32 = "0123456789abcdef0123456789abcdef";
        assert_eq!(s32.len(), 32);
        assert_eq!(escape(s32), serde_json::to_string(s32).unwrap());

        // 32 bytes with escapes at different positions
        let s32_esc = "0123456789abcde\"0123456789abcde";
        assert_eq!(escape(s32_esc), serde_json::to_string(s32_esc).unwrap());
    }

    #[test]
    fn test_large_strings_128_bytes() {
        // Exactly 128 bytes - main loop size
        let s128 = "0123456789abcdef".repeat(8);
        assert_eq!(s128.len(), 128);
        assert_eq!(escape(&s128), serde_json::to_string(&s128).unwrap());

        // 128 bytes with escapes spread throughout
        let mut s128_esc = String::new();
        for i in 0..8 {
            if i % 2 == 0 {
                s128_esc.push_str("0123456789abcd\n");
            } else {
                s128_esc.push_str("0123456789abcd\"");
            }
        }
        assert_eq!(escape(&s128_esc), serde_json::to_string(&s128_esc).unwrap());
    }

    #[test]
    fn test_unaligned_data() {
        // Test strings that start at various alignments
        for offset in 0..32 {
            let padding = " ".repeat(offset);
            let test_str = format!("{}{}", padding, "test\nstring\"with\\escapes");
            let result = escape(&test_str[offset..]);
            let expected = serde_json::to_string(&test_str[offset..]).unwrap();
            assert_eq!(result, expected, "Failed at offset {}", offset);
        }
    }

    #[test]
    fn test_sparse_escapes() {
        // Large string with escapes only at the beginning and end
        let mut s = String::new();
        s.push('"');
        s.push_str(&"a".repeat(500));
        s.push('\\');
        assert_eq!(escape(&s), serde_json::to_string(&s).unwrap());
    }

    #[test]
    fn test_dense_escapes() {
        // String with many escapes
        let s = "\"\\\"\\\"\\\"\\".repeat(50);
        assert_eq!(escape(&s), serde_json::to_string(&s).unwrap());

        // All control characters
        let mut ctrl = String::new();
        for _ in 0..10 {
            for i in 0u8..32 {
                ctrl.push(i as char);
            }
        }
        assert_eq!(escape(&ctrl), serde_json::to_string(&ctrl).unwrap());
    }

    #[test]
    fn test_boundary_conditions() {
        // Test around 256 byte boundary (common cache line multiple)
        for size in 250..260 {
            let s = "a".repeat(size);
            assert_eq!(escape(&s), serde_json::to_string(&s).unwrap());

            // With escape at the end
            let mut s_esc = "a".repeat(size - 1);
            s_esc.push('"');
            assert_eq!(escape(&s_esc), serde_json::to_string(&s_esc).unwrap());
        }
    }

    #[test]
    fn test_all_escape_types() {
        // Test each escape type individually
        assert_eq!(escape("\x00"), r#""\u0000""#);
        assert_eq!(escape("\x08"), r#""\b""#);
        assert_eq!(escape("\x09"), r#""\t""#);
        assert_eq!(escape("\x0A"), r#""\n""#);
        assert_eq!(escape("\x0C"), r#""\f""#);
        assert_eq!(escape("\x0D"), r#""\r""#);
        assert_eq!(escape("\x1F"), r#""\u001f""#);
        assert_eq!(escape("\""), r#""\"""#);
        assert_eq!(escape("\\"), r#""\\""#);

        // Test all control characters
        for i in 0u8..32 {
            let s = String::from_utf8(vec![i]).unwrap();
            let result = escape(&s);
            let expected = String::from_utf8(QUOTE_TAB[i as usize].1.to_vec())
                .unwrap()
                .trim_end_matches('\0')
                .to_string();
            assert_eq!(
                result,
                format!("\"{}\"", expected),
                "Failed for byte 0x{:02x}",
                i
            );
        }
    }

    #[test]
    fn test_mixed_content() {
        // Mix of ASCII, escapes, and multi-byte UTF-8
        let mixed = r#"Hello "World"!
    Tab:	Here
    Emoji: 😀 Chinese: 中文
    Math: ∑∫∂ Music: 𝄞
    Escape: \" \\ \n \r \t"#;
        assert_eq!(escape(mixed), serde_json::to_string(mixed).unwrap());
    }

    #[test]
    fn test_repeated_patterns() {
        // Patterns that might benefit from or confuse SIMD operations
        let pattern1 = "abcd".repeat(100);
        assert_eq!(escape(&pattern1), serde_json::to_string(&pattern1).unwrap());

        let pattern2 = "a\"b\"".repeat(100);
        assert_eq!(escape(&pattern2), serde_json::to_string(&pattern2).unwrap());

        let pattern3 = "\t\n".repeat(100);
        assert_eq!(escape(&pattern3), serde_json::to_string(&pattern3).unwrap());
    }

    #[test]
    fn test_rxjs() {
        let mut sources = Vec::new();
        read_dir_recursive("node_modules/rxjs/src", &mut sources, |p| {
            matches!(p.extension().and_then(|e| e.to_str()), Some("ts"))
        })
        .unwrap();
        assert!(!sources.is_empty());
        sources.shuffle(&mut rand::rng());
        for source in sources
            .iter()
            .take(if cfg!(miri) { 10 } else { sources.len() })
        {
            assert_eq!(escape(source), serde_json::to_string(&source).unwrap());
            let mut output = String::with_capacity(source.len() * 6 + 32 + 3);
            escape_into(source, unsafe { output.as_mut_vec() });
            assert_eq!(output, serde_json::to_string(&source).unwrap());
        }
    }

    #[test]
    fn test_sources() {
        for source in load_affine_sources().unwrap() {
            assert_eq!(escape(&source), serde_json::to_string(&source).unwrap());
            let mut output = String::with_capacity(source.len() * 6 + 32 + 3);
            escape_into(&source, unsafe { output.as_mut_vec() });
            assert_eq!(output, serde_json::to_string(&source).unwrap());
        }
    }

    fn load_affine_sources() -> Result<impl Iterator<Item = String>, std::io::Error> {
        let mut sources = Vec::new();
        read_dir_recursive("fixtures", &mut sources, |p| {
            matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("ts") | Some("tsx") | Some("js") | Some("mjs") | Some("cjs")
            )
        })?;
        assert!(!sources.is_empty());
        let len = sources.len();
        sources.shuffle(&mut rand::rng());
        Ok(sources.into_iter().take(if cfg!(miri) { 10 } else { len }))
    }

    fn read_dir_recursive<P: AsRef<Path>, F: Fn(PathBuf) -> bool + Copy>(
        dir: P,
        sources: &mut Vec<String>,
        f: F,
    ) -> Result<(), std::io::Error> {
        let dir = read_dir(dir)?;
        for entry in dir {
            let p = entry?;
            let metadata = std::fs::metadata(p.path())?;
            if metadata.is_file() && f(p.path()) {
                sources.push(std::fs::read_to_string(p.path())?);
            }
            if metadata.is_dir() {
                read_dir_recursive(p.path(), sources, f)?;
            }
        }
        Ok(())
    }
}
