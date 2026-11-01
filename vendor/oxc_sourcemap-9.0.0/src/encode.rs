//! Ported and modified from <https://github.com/getsentry/rust-sourcemap/blob/9.1.0/src/encoder.rs>

use std::ops::{Deref, DerefMut};

use json_escape_simd::escape_into;

use crate::JSONSourceMap;
use crate::{SourceMap, Token, token::TokenChunk};

pub fn encode(sourcemap: &SourceMap<'_>) -> JSONSourceMap {
    JSONSourceMap {
        version: 3,
        file: sourcemap.get_file().map(ToString::to_string),
        mappings: {
            let mut mappings = String::with_capacity(estimate_mappings_length(sourcemap));
            serialize_sourcemap_mappings(sourcemap, &mut mappings);
            mappings
        },
        source_root: sourcemap.get_source_root().map(ToString::to_string),
        sources: sourcemap.sources.iter().map(ToString::to_string).collect(),
        sources_content: sourcemap.source_contents.iter().any(Option::is_some).then(|| {
            sourcemap.get_source_contents().map(|content| content.map(str::to_owned)).collect()
        }),
        names: sourcemap.names.iter().map(ToString::to_string).collect(),
        debug_id: sourcemap.get_debug_id().map(ToString::to_string),
        ignore_list: sourcemap.ignore_list.clone(),
    }
}

pub fn encode_to_string(sourcemap: &SourceMap<'_>) -> String {
    encode_to_string_with_null_contents(sourcemap, false)
}

pub fn encode_to_string_with_null_contents(
    sourcemap: &SourceMap<'_>,
    keep_null_contents: bool,
) -> String {
    // Worst-case capacity accounting:
    // - escape_into may write up to (len * 2 + 2) for each string
    // - include commas between items and constant JSON punctuation/keys
    let mut max_segments = 0usize;

    // {"version":3,
    max_segments += 13;

    // Optional "file":"...",
    if let Some(file) = sourcemap.get_file() {
        max_segments += 8 /* "file": */ + file.len() * 6 + 2 /* quotes */ + 1 /* , */;
    }

    // Optional "sourceRoot":"...",
    if let Some(source_root) = sourcemap.get_source_root() {
        max_segments += 14 /* "sourceRoot": */ + source_root.len() * 6 + 2 /* quotes */ + 1 /* , */;
    }

    // Calculate string lengths in a single pass for better cache locality
    let names_count = sourcemap.names.len();
    let sources_count = sourcemap.sources.len();
    // Accumulate total string bytes across all collections
    let mut total_string_bytes = 0usize;

    for name in &sourcemap.names {
        total_string_bytes += name.len();
    }

    for source in &sourcemap.sources {
        total_string_bytes += source.len();
    }

    // Single pass over source_contents to check existence and accumulate byte lengths
    let (has_some_contents, sc_bytes) =
        sourcemap.source_contents.iter().fold((false, 0usize), |(has_some, bytes), content| {
            match content {
                Some(s) => (true, bytes + s.len()),
                None => (has_some, bytes + 4), // "null"
            }
        });
    let has_source_contents =
        has_some_contents || (keep_null_contents && !sourcemap.source_contents.is_empty());
    total_string_bytes += sc_bytes;
    let sc_count = if has_source_contents { sourcemap.source_contents.len() } else { 0 };

    // Calculate total capacity needed (realistic ~1.125x headroom for escaping instead of 6x
    // worst-case, avoiding 500+ MB over-allocations on large sourcemaps).
    max_segments += 9 + 13; // "names":[ + ],"sources":[
    if has_source_contents {
        max_segments += 20; // ],"sourcesContent":[
    }
    max_segments += total_string_bytes + (total_string_bytes >> 3) + 64;
    max_segments += 2 * (names_count + sources_count + sc_count); // quotes around each item

    // Commas between array items
    let comma_count = names_count.saturating_sub(1)
        + sources_count.saturating_sub(1)
        + sc_count.saturating_sub(1);
    max_segments += comma_count;

    // Optional ],"ignoreList":[
    if let Some(ignore_list) = &sourcemap.ignore_list {
        max_segments += 16; // ],"ignoreList":[

        let ig_count = ignore_list.len();
        // At most 10 digits per u32, plus commas between items.
        max_segments += 10 * ig_count + ig_count.saturating_sub(1);
    }

    // ],"mappings":"
    max_segments += 14;
    let est_mappings_len = estimate_mappings_length(sourcemap);
    max_segments += est_mappings_len;

    // Optional ,"debugId":<escaped>
    if let Some(debug_id) = sourcemap.get_debug_id() {
        max_segments += 12 /* ,"debugId": */ + debug_id.len() * 6 + 2 /* quotes */;
    }

    // "} (closing quote of mappings + closing brace)
    max_segments += 2;
    let mut contents = PreAllocatedString::new(max_segments);

    contents.push("{\"version\":3,");
    if let Some(file) = sourcemap.get_file() {
        contents.push("\"file\":");
        escape_into(file, contents.as_mut_vec());
        contents.push(",");
    }

    if let Some(source_root) = sourcemap.get_source_root() {
        contents.push("\"sourceRoot\":");
        escape_into(source_root, contents.as_mut_vec());
        contents.push(",");
    }

    let parallel_large = sourcemap.tokens.len() >= 50_000
        && sourcemap.token_chunks.as_ref().is_some_and(|c| c.len() >= 8);

    if parallel_large {
        let token_chunks = sourcemap.token_chunks.as_ref().unwrap();
        let sc_slice = if has_source_contents {
            sourcemap.source_contents.as_slice()
        } else {
            &[]
        };
        let sc_parts_count = if sc_slice.len() >= 64 { 4 } else { 1 };
        let map_parts_count = 4usize.min(token_chunks.len());
        let tokens_ref = &sourcemap.tokens;

        let (sc_parts, map_parts) = std::thread::scope(|s| {
            let mut sc_handles = Vec::with_capacity(sc_parts_count);
            if !sc_slice.is_empty() {
                let chunk_size = sc_slice.len().div_ceil(sc_parts_count);
                for part in sc_slice.chunks(chunk_size) {
                    sc_handles.push(s.spawn(move || {
                        let part_bytes: usize = part
                            .iter()
                            .map(|c| c.as_ref().map_or(4, |s| s.len() + (s.len() >> 3) + 4))
                            .sum();
                        let mut buf = Vec::with_capacity(part_bytes);
                        let mut iter = part.iter();
                        if let Some(first) = iter.next() {
                            match first {
                                Some(st) => escape_into(&**st, &mut buf),
                                None => buf.extend_from_slice(b"null"),
                            }
                            for item in iter {
                                buf.push(b',');
                                match item {
                                    Some(st) => escape_into(&**st, &mut buf),
                                    None => buf.extend_from_slice(b"null"),
                                }
                            }
                        }
                        buf
                    }));
                }
            }

            let mut map_handles = Vec::with_capacity(map_parts_count);
            let tc_chunk_size = token_chunks.len().div_ceil(map_parts_count);
            for tc_part in token_chunks.chunks(tc_chunk_size) {
                let est = est_mappings_len / map_parts_count + 1024;
                map_handles.push(s.spawn(move || {
                    let mut out = String::with_capacity(est);
                    for tc in tc_part {
                        serialize_mappings(tokens_ref, tc, &mut out);
                    }
                    out
                }));
            }

            contents.push("\"names\":[");
            contents.push_list(sourcemap.names.iter(), |st, out| escape_into(&**st, out));

            contents.push("],\"sources\":[");
            contents.push_list(sourcemap.sources.iter(), |st, out| escape_into(&**st, out));

            let sc_res: Vec<Vec<u8>> =
                sc_handles.into_iter().map(|h| h.join().unwrap()).collect();
            let map_res: Vec<String> =
                map_handles.into_iter().map(|h| h.join().unwrap()).collect();
            (sc_res, map_res)
        });

        if has_source_contents {
            contents.push("],\"sourcesContent\":[");
            let out_vec = contents.as_mut_vec();
            let mut first = true;
            for part in sc_parts {
                if part.is_empty() {
                    continue;
                }
                if !first {
                    out_vec.push(b',');
                }
                first = false;
                out_vec.extend_from_slice(&part);
            }
        }

        if let Some(ignore_list) = &sourcemap.ignore_list {
            contents.push("],\"ignoreList\":[");
            contents.push_list(ignore_list.iter(), |st, output| {
                output.extend_from_slice(st.to_string().as_bytes());
            });
        }

        contents.push("],\"mappings\":\"");
        for part in map_parts {
            contents.push(&part);
        }
        contents.push("\"");
    } else {
        contents.push("\"names\":[");
        contents.push_list(sourcemap.names.iter(), |s, out| escape_into(&**s, out));

        contents.push("],\"sources\":[");
        contents.push_list(sourcemap.sources.iter(), |s, out| escape_into(&**s, out));

        if has_source_contents {
            let source_contents = &sourcemap.source_contents;
            contents.push("],\"sourcesContent\":[");
            contents.push_list(source_contents.iter(), |v, output| match v {
                Some(s) => escape_into(&**s, output),
                None => output.extend_from_slice(b"null"),
            });
        }

        if let Some(ignore_list) = &sourcemap.ignore_list {
            contents.push("],\"ignoreList\":[");
            contents.push_list(ignore_list.iter(), |s, output| {
                output.extend_from_slice(s.to_string().as_bytes());
            });
        }

        contents.push("],\"mappings\":\"");
        serialize_sourcemap_mappings(sourcemap, &mut contents);
        contents.push("\"");
    }

    if let Some(debug_id) = sourcemap.get_debug_id() {
        contents.push(",\"debugId\":");
        escape_into(debug_id, contents.as_mut_vec());
    }

    contents.push("}");

    contents.consume()
}

fn estimate_mappings_length(sourcemap: &SourceMap<'_>) -> usize {
    sourcemap
        .token_chunks
        .as_ref()
        .map(|chunks| {
            // Increased from 10 to 12 to account for worst-case VLQ encoding and separators.
            // Each chunk also writes one semicolon per destination line it advances.
            chunks
                .iter()
                .map(|t| {
                    let lines = (t.start < t.end).then(|| {
                        sourcemap.tokens[t.end as usize - 1]
                            .get_dst_line()
                            .saturating_sub(t.prev_dst_line)
                    });
                    (t.end - t.start) as usize * 12 + lines.unwrap_or(0) as usize
                })
                .sum::<usize>()
        })
        .unwrap_or_else(|| {
            sourcemap.tokens.len() * 12 + sourcemap.tokens.last().map_or(0, |t| t.dst_line as usize)
        })
}

fn serialize_sourcemap_mappings(sm: &SourceMap<'_>, output: &mut String) {
    if let Some(token_chunks) = sm.token_chunks.as_ref() {
        token_chunks.iter().for_each(|token_chunk| {
            serialize_mappings(&sm.tokens, token_chunk, output);
        })
    } else {
        serialize_mappings(
            &sm.tokens,
            &TokenChunk::new(0, sm.tokens.len() as u32, 0, 0, 0, 0, 0, 0),
            output,
        );
    }
}

// Max length of a single VLQ encoding
const MAX_VLQ_BYTES: usize = 7;

fn serialize_mappings(tokens: &[Token], token_chunk: &TokenChunk, output: &mut String) {
    let TokenChunk {
        start,
        end,
        mut prev_dst_line,
        mut prev_dst_col,
        mut prev_src_line,
        mut prev_src_col,
        mut prev_name_id,
        mut prev_source_id,
    } = *token_chunk;

    let mut need_comma = start != 0;

    for token in &tokens[start as usize..end as usize] {
        // Max length of a single VLQ encoding is 7 bytes. Max number of calls to `encode_vlq` is 5.
        // Also need 1 byte for each line number difference, or 1 byte if no line num difference.
        // Reserve this amount of capacity in `rv` early, so can skip bounds checks in code below.
        // As well as skipping the bounds checks, this also removes a function call to
        // `alloc::raw_vec::RawVec::grow_one` for every byte that's pushed.
        // https://godbolt.org/z/44G8jjss3
        const MAX_TOTAL_VLQ_BYTES: usize = 5 * MAX_VLQ_BYTES;

        let num_line_breaks = token.get_dst_line() - prev_dst_line;
        if num_line_breaks != 0 {
            let required = MAX_TOTAL_VLQ_BYTES + num_line_breaks as usize;
            if output.capacity() - output.len() < required {
                output.reserve(required);
            }
            // SAFETY: We have reserved sufficient capacity for `num_line_breaks` bytes
            unsafe { push_bytes_unchecked(output, b';', num_line_breaks) };
            prev_dst_col = 0;
            prev_dst_line += num_line_breaks;
        } else if need_comma {
            let required = MAX_TOTAL_VLQ_BYTES + 1;
            if output.capacity() - output.len() < required {
                output.reserve(required);
            }
            // SAFETY: We have reserved sufficient capacity for 1 byte
            unsafe { push_byte_unchecked(output, b',') };
        } else {
            // First token of the first chunk: no delimiter is emitted, but the
            // five VLQ fields below still write up to 35 bytes, and the
            // caller's estimate can be tighter than that (~12 bytes/token), so
            // capacity must still be ensured here.
            if output.capacity() - output.len() < MAX_TOTAL_VLQ_BYTES {
                output.reserve(MAX_TOTAL_VLQ_BYTES);
            }
        }

        if let Some(source_id) = token.get_source_id() {
            // The dominant segment shape in real bundler output is 4 or 5
            // single-digit VLQ fields (small deltas). Transform all field
            // deltas up front so that shape can be detected with one test and
            // written with a single 8-byte store, instead of running the
            // digit-at-a-time VLQ loop per field.
            let v0 = vlq_value(token.get_dst_col(), prev_dst_col);
            let v1 = vlq_value(source_id, prev_source_id);
            let v2 = vlq_value(token.get_src_line(), prev_src_line);
            let v3 = vlq_value(token.get_src_col(), prev_src_col);
            prev_dst_col = token.get_dst_col();
            prev_source_id = source_id;
            prev_src_line = token.get_src_line();
            prev_src_col = token.get_src_col();

            if let Some(name_id) = token.get_name_id() {
                let v4 = vlq_value(name_id, prev_name_id);
                prev_name_id = name_id;
                if !try_push_fast_4_or_5_segment(output, [v0, v1, v2, v3, v4]) {
                    // SAFETY: `MAX_TOTAL_VLQ_BYTES` (35) spare capacity was
                    // ensured above — five `encode_vlq` calls need 7 bytes each.
                    unsafe {
                        encode_vlq(output, v0);
                        encode_vlq(output, v1);
                        encode_vlq(output, v2);
                        encode_vlq(output, v3);
                        encode_vlq(output, v4);
                    }
                }
            } else if !try_push_fast_4_or_5_segment(output, [v0, v1, v2, v3]) {
                // SAFETY: same as above, with only four fields.
                unsafe {
                    encode_vlq(output, v0);
                    encode_vlq(output, v1);
                    encode_vlq(output, v2);
                    encode_vlq(output, v3);
                }
            }
        } else {
            // SAFETY: `MAX_TOTAL_VLQ_BYTES` spare capacity was ensured above.
            unsafe { encode_vlq(output, vlq_value(token.get_dst_col(), prev_dst_col)) };
            prev_dst_col = token.get_dst_col();
        }

        need_comma = true;
    }
}

/// Transform the diff `a - b` into its VLQ integer representation: the sign
/// goes in the low bit and the remaining bits are the magnitude (the
/// branchless inverse of `decode_sign` in `decode.rs`).
/// The result encodes to a single base64 char iff it fits the 5 payload bits
/// of one char, i.e. no bit at or above the continuation bit is set
/// (`value & !0x1F == 0`).
#[inline]
fn vlq_value(a: u32, b: u32) -> u64 {
    (u64::from(a.abs_diff(b)) << 1) | u64::from(a < b)
}

/// Fast path for the dominant sourcemap segment shapes: a 4- or 5-field
/// segment whose VLQ values are all single-digit (no continuation bit).
/// Pushes all fields' base64 chars with a single unaligned 8-byte store and
/// one length update, and returns `true`. Returns `false` without writing
/// anything when any field needs more than one char (or, in theory, when
/// `out` lacks 8 bytes of spare capacity — never the case in
/// `serialize_mappings`, which reserves 35 bytes per token), so the caller
/// can fall back to the per-digit `encode_vlq` loop.
#[inline]
fn try_push_fast_4_or_5_segment<const N: usize>(out: &mut String, vals: [u64; N]) -> bool {
    const { assert!(N <= 8) };

    let mut all = 0u64;
    for &v in &vals {
        all |= v;
    }
    // A value with any bit at or above the VLQ continuation bit needs more
    // than one base64 char. This is the u64-wide analogue of the `& 0xE0`
    // test in `decode.rs` — `!0x1F` rather than `0xE0` because these are
    // full VLQ integers, not single decoded base64 bytes.
    if all & !0x1F != 0 {
        return false;
    }

    // The store below always writes 8 bytes (only the first `N` become
    // visible via `set_len`). The caller reserves more than this for the
    // fallback path anyway, so this predictable branch never fails.
    if out.capacity() - out.len() < 8 {
        return false;
    }

    let mut packed = 0u64;
    for (i, &v) in vals.iter().enumerate() {
        // `& 0x1F` keeps the index provably in bounds for the 64-entry table;
        // it is a no-op here since the mask test above established `v < 32`.
        packed |= u64::from(B64_CHARS.0[(v & 0x1F) as usize]) << (i * 8);
    }

    // SAFETY: the capacity check above guarantees 8 spare bytes for the
    // 8-byte store. All bytes in `B64_CHARS` are ASCII, so the buffer stays
    // valid UTF-8.
    unsafe {
        let vec = out.as_mut_vec();
        let len = vec.len();
        let ptr = vec.as_mut_ptr().add(len);
        // `to_le` so the first char lands at the lowest address on any target.
        ptr.cast::<u64>().write_unaligned(packed.to_le());
        vec.set_len(len + N);
    }
    true
}

// Align chars lookup table on 64 so occupies a single cache line
#[repr(align(64))]
struct Aligned64([u8; 64]);

static B64_CHARS: Aligned64 =
    Aligned64(*b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/");

/// Encode a VLQ integer representation (see [`vlq_value`]) and push encoding
/// into `out`. Will push between 1 byte (num = 0) and 7 bytes (num from a
/// diff of -u32::MAX).
///
/// # SAFETY
/// Caller must ensure at least 7 bytes spare capacity in `out`,
/// as this function does not perform any bounds checks.
unsafe fn encode_vlq(out: &mut String, mut num: u64) {
    unsafe {
        // Breaking out of loop early when have reached last char (rather than conditionally adding
        // 32 for last char within the loop) removes 3 instructions from the loop.
        // https://godbolt.org/z/Es4Pavh9j
        // This translates to a 16% speed-up for VLQ encoding.
        // (A packed-u64 variant with a single 8-byte store and one length
        // update was benchmarked at ~25% slower on serialize: it adds ALU work
        // to the dominant single-byte case, and the per-byte length update
        // does not stall in practice.)
        let mut digit;
        loop {
            digit = num & 0b11111;
            num >>= 5;
            if num == 0 {
                break;
            }

            let b = B64_CHARS.0[digit as usize + 32];
            // SAFETY:
            // * This loop can execute a maximum of 7 times, and on last turn will exit before getting here.
            //   Caller promises there are at least 7 bytes spare capacity in `out` at start. We only
            //   push 1 byte on each turn, so guaranteed there is at least 1 byte capacity in `out` here.
            // * All values in `B64_CHARS` lookup table are ASCII bytes.
            push_byte_unchecked(out, b);
        }

        let b = B64_CHARS.0[digit as usize];
        // SAFETY:
        // * The loop above pushes max 6 bytes. Caller promises there are at least 7 bytes spare capacity
        //   in `out` at start. So guaranteed there is at least 1 byte capacity in `out` here.
        // * All values in `B64_CHARS` lookup table are ASCII bytes.
        push_byte_unchecked(out, b);
    }
}

/// Push a byte to `out` without bounds checking.
///
/// # SAFETY
/// * `out` must have at least 1 byte spare capacity.
/// * `b` must be an ASCII byte (i.e. not `>= 128`).
//
// `#[inline(always)]` to ensure that `len` is stored in a register during `encode_vlq`'s loop.
#[expect(clippy::inline_always)]
#[inline(always)]
unsafe fn push_byte_unchecked(out: &mut String, b: u8) {
    unsafe {
        debug_assert!(out.len() < out.capacity());
        debug_assert!(b.is_ascii());

        let out = out.as_mut_vec();
        let len = out.len();
        let ptr = out.as_mut_ptr().add(len);
        ptr.write(b);
        out.set_len(len + 1);
    }
}

/// Push a byte to `out` a number of times without bounds checking.
///
/// # SAFETY
/// * `out` must have at least `repeats` bytes spare capacity.
/// * `b` must be an ASCII byte (i.e. not `>= 128`).
#[inline]
unsafe fn push_bytes_unchecked(out: &mut String, b: u8, repeats: u32) {
    unsafe {
        debug_assert!(out.capacity() - out.len() >= repeats as usize);
        debug_assert!(b.is_ascii());

        let out = out.as_mut_vec();
        let len = out.len();
        let mut ptr = out.as_mut_ptr().add(len);
        for _ in 0..repeats {
            ptr.write(b);
            ptr = ptr.add(1);
        }
        out.set_len(len + repeats as usize);
    }
}

/// A helper for pre-allocate string buffer.
///
/// Pre-allocate a Cow<'a, str> buffer, and push the segment into it.
/// Finally, convert it to a pre-allocated length String.
#[repr(transparent)]
struct PreAllocatedString(String);

impl Deref for PreAllocatedString {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PreAllocatedString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl PreAllocatedString {
    fn new(max_segments: usize) -> Self {
        Self(String::with_capacity(max_segments))
    }

    #[inline]
    fn push(&mut self, s: &str) {
        self.0.push_str(s);
    }

    #[inline]
    fn push_list<S, I>(&mut self, mut iter: I, encode: impl Fn(S, &mut Vec<u8>))
    where
        I: Iterator<Item = S>,
    {
        let Some(first) = iter.next() else {
            return;
        };
        encode(first, self.as_mut_vec());

        for other in iter {
            self.0.push(',');
            encode(other, self.as_mut_vec());
        }
    }

    fn as_mut_vec(&mut self) -> &mut Vec<u8> {
        // SAFETY: we are sure that the string is not shared
        unsafe { self.0.as_mut_vec() }
    }

    #[inline]
    fn consume(self) -> String {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_empty_ignore_list() {
        let mut sm = SourceMap::default();
        sm.set_ignore_list(vec![]);
        assert_eq!(
            sm.to_json_string(),
            r#"{"version":3,"names":[],"sources":[],"ignoreList":[],"mappings":""}"#
        );
    }

    #[test]
    fn encode_ignore_list() {
        for fields in [
            "",
            r#", "ignoreList": []"#,
            r#", "ignoreList": [0]"#,
            r#", "x_google_ignoreList": []"#,
            r#", "x_google_ignoreList": [0]"#,
            r#", "ignoreList": [], "x_google_ignoreList": [0]"#,
        ] {
            let input = format!(r#"{{"version":3,"sources":["a.js"],"mappings":""{fields}}}"#);
            let sm = SourceMap::from_json_string(&input).unwrap();
            assert_eq!(sm.to_json().ignore_list.as_deref(), sm.get_ignore_list());

            let encoded = sm.to_json_string();
            let json: serde_json::Value = serde_json::from_str(&encoded).unwrap();
            assert!(json.get("x_google_ignoreList").is_none());
            assert_eq!(
                json.get("ignoreList"),
                sm.get_ignore_list().map(|list| serde_json::json!(list)).as_ref()
            );
            let decoded = SourceMap::from_json_string(&encoded).unwrap();
            assert_eq!(decoded.get_ignore_list(), sm.get_ignore_list());
        }
    }

    #[test]
    fn encode_roundtrip() {
        let input = r#"{
        "version": 3,
        "sources": ["coolstuff.js"],
        "sourceRoot": "x",
        "names": ["x","alert"],
        "mappings": "AAAA,GAAIA,GAAI,EACR,IAAIA,GAAK,EAAG,CACVC,MAAM",
        "ignoreList": [0]
    }"#;
        let sm = SourceMap::from_json_string(input).unwrap();
        let encoded = sm.to_json_string();
        let sm2 = SourceMap::from_json_string(&encoded).unwrap();

        for (tok1, tok2) in sm.get_tokens().zip(sm2.get_tokens()) {
            assert_eq!(tok1, tok2);
        }

        // spellchecker:off
        let input = r#"{
        "version": 3,
        "file": "index.js",
        "names": [
            "text",
            "text"
        ],
        "sources": [
            "../../hmr.js",
            "../../main.js",
            "../../index.html"
        ],
        "sourcesContent": [
            "export const foo = 'hello'\n\ntext('.hmr', foo)\n\nfunction text(el, text) {\n  document.querySelector(el).textContent = text\n}\n\nimport.meta.hot?.accept((mod) =\u003E {\n  if (mod) {\n    text('.hmr', mod.foo)\n  }\n})\n",
            "import './hmr.js'\n\ntext('.app', 'hello')\n\nfunction text(el, text) {\n  document.querySelector(el).textContent = text\n}\n",
            "\u003Ch1\u003EHMR Full Bundle Mode\u003C/h1\u003E\n\n\u003Cdiv class=\"app\"\u003E\u003C/div\u003E\n\u003Cdiv class=\"hmr\"\u003E\u003C/div\u003E\n\n\u003Cscript type=\"module\" src=\"./main.js\"\u003E\u003C/script\u003E\n"
        ],
        "mappings": ";;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;AAAA,MAAa,MAAM;AAEnBA,OAAK,QAAQ,IAAI;AAEjB,SAASA,OAAK,IAAI,QAAM;AACtB,UAAS,cAAc,GAAG,CAAC,cAAcA;;SAG1B,QAAQ,QAAQ;AAC/B,KAAI,KAAK;AACP,SAAK,QAAQ,IAAI,IAAI;;EAEvB;;;;;;;ACVF,KAAK,QAAQ,QAAQ;AAErB,SAAS,KAAK,IAAI,QAAM;AACtB,UAAS,cAAc,GAAG,CAAC,cAAcC"
    }"#;
        // spellchecker:on
        let sm = SourceMap::from_json_string(input).unwrap();
        let encoded = sm.to_json_string();
        let sm2 = SourceMap::from_json_string(&encoded).unwrap();

        for (tok1, tok2) in sm.get_tokens().zip(sm2.get_tokens()) {
            assert_eq!(tok1, tok2);
        }
    }

    #[test]
    fn encode_escape_string() {
        // '\0' should be escaped.
        let mut sm = SourceMap::new(
            None,
            vec!["name_length_greater_than_16_\0".into()],
            None,
            vec!["\0".into()],
            vec![Some("emoji-👀-\0".into())],
            vec![].into_boxed_slice(),
            None,
        );
        sm.set_ignore_list(vec![0]);
        sm.set_debug_id("56431d54-c0a6-451d-8ea2-ba5de5d8ca2e");
        assert_eq!(
            sm.to_json_string(),
            r#"{"version":3,"names":["name_length_greater_than_16_\u0000"],"sources":["\u0000"],"sourcesContent":["emoji-👀-\u0000"],"ignoreList":[0],"mappings":"","debugId":"56431d54-c0a6-451d-8ea2-ba5de5d8ca2e"}"#
        );
    }

    #[test]
    fn vlq_encode_diff() {
        // Most important tests here are that with maximum values, `encode_vlq` pushes maximum of 7 bytes.
        // This invariant is essential to safety of `encode_vlq`.
        #[rustfmt::skip]
    const FIXTURES: &[(u32, u32, &str)] = &[
        (0,           0, "A"),
        (1,           0, "C"),
        (2,           0, "E"),
        (15,          0, "e"),
        (16,          0, "gB"),
        (511,         0, "+f"),
        (512,         0, "ggB"),
        (16_383,      0, "+/f"),
        (16_384,      0, "gggB"),
        (524_287,     0, "+//f"),
        (524_288,     0, "ggggB"),
        (16_777_215,  0, "+///f"),
        (16_777_216,  0, "gggggB"),
        (536_870_911, 0, "+////f"),
        (536_870_912, 0, "ggggggB"),
        (u32::MAX,    0, "+/////H"), // 7 bytes

        (0, 1,           "D"),
        (0, 2,           "F"),
        (0, 15,          "f"),
        (0, 16,          "hB"),
        (0, 511,         "/f"),
        (0, 512,         "hgB"),
        (0, 16_383,      "//f"),
        (0, 16_384,      "hggB"),
        (0, 524_287,     "///f"),
        (0, 524_288,     "hgggB"),
        (0, 16_777_215,  "////f"),
        (0, 16_777_216,  "hggggB"),
        (0, 536_870_911, "/////f"),
        (0, 536_870_912, "hgggggB"),
        (0, u32::MAX,    "//////H"), // 7 bytes
    ];

        for (a, b, res) in FIXTURES.iter().copied() {
            let mut out = String::with_capacity(MAX_VLQ_BYTES);
            // SAFETY: `out` has 7 bytes spare capacity
            unsafe { encode_vlq(&mut out, vlq_value(a, b)) };
            assert_eq!(&out, res);
        }
    }

    #[test]
    fn encode_fast_4_or_5_segment() {
        // Exercises the packed single-store fast path and its boundary with the
        // per-digit slow path: deltas of ±15 are single-digit (fast), ±16 are
        // not (slow), and negative deltas must keep the sign in the VLQ LSB.
        let tokens = vec![
            Token::new(0, 15, 15, 15, Some(0), Some(0)), // all-small, fast (5 fields)
            Token::new(0, 30, 0, 0, Some(0), None),      // negative deltas, fast (4 fields)
            Token::new(0, 46, 16, 16, Some(0), Some(0)), // +16 src deltas, slow
            Token::new(0, 47, 0, 0, Some(0), Some(0)),   // -16 src deltas, slow
            Token::new(1, 1, 1, 1, Some(0), Some(0)),    // after line break, fast
            Token::new(1, 2, 1, 1, None, None),          // no source, dst_col only
        ];
        let sm = SourceMap::new(
            None,
            vec!["a".into()],
            None,
            vec!["a.js".into()],
            vec![],
            tokens.into_boxed_slice(),
            None,
        );
        assert_eq!(sm.to_json().mappings, "eAeeA,eAff,gBAgBgBA,CAhBhBA;CACCA,C"); // spellchecker:disable-line
        let encoded = sm.to_json_string();
        let reparsed = SourceMap::from_json_string(&encoded).unwrap();
        assert!(sm.get_tokens().eq(reparsed.get_tokens()));
    }

    #[test]
    fn encode_all_sources_content_null() {
        let sm = SourceMap::new(
            None,
            vec![],
            None,
            vec!["a.js".into(), "b.js".into()],
            vec![None, None],
            vec![].into_boxed_slice(),
            None,
        );
        let json = sm.to_json_string();
        assert!(
            !json.contains("sourcesContent"),
            "sourcesContent should be omitted when all items are None"
        );

        let json_map = encode(&sm);
        assert!(json_map.sources_content.is_none());

        let sm = SourceMap::new(
            None,
            vec![],
            None,
            vec!["a.js".into(), "b.js".into()],
            vec![Some("content".into()), None],
            vec![].into_boxed_slice(),
            None,
        );
        let json = sm.to_json_string();
        assert!(
            json.contains("sourcesContent"),
            "sourcesContent should be present when at least one item is Some"
        );
        assert!(
            json.contains(r#""sourcesContent":["content",null]"#),
            "None source_contents should be encoded as raw null, not quoted \"null\""
        );

        let json_map = encode(&sm);
        assert!(json_map.sources_content.is_some());
    }

    #[test]
    fn encode_escape_file_and_source_root() {
        let sm = SourceMap::new(
            Some("file\0name.js".into()),
            vec![],
            Some("root\0path".into()),
            vec![],
            vec![],
            vec![].into_boxed_slice(),
            None,
        );
        let json = sm.to_json_string();
        assert!(
            json.contains(r#""file":"file\u0000name.js""#),
            "file field should have \\0 escaped: {json}"
        );
        assert!(
            json.contains(r#""sourceRoot":"root\u0000path""#),
            "sourceRoot field should have \\0 escaped: {json}"
        );
        // Verify the output is valid JSON by round-tripping
        SourceMap::from_json_string(&json).unwrap();
    }

    #[test]
    fn encode_escape_debug_id() {
        let mut sm = SourceMap::default();
        // A debug_id containing JSON-special characters must be escaped, otherwise
        // the output is malformed JSON.
        sm.set_debug_id("id-with-\"quote\"-and-\\backslash");
        let json = sm.to_json_string();
        // Round-trip must succeed (would fail if the quote isn't escaped).
        let roundtripped = SourceMap::from_json_string(&json).unwrap();
        assert_eq!(roundtripped.get_debug_id(), Some("id-with-\"quote\"-and-\\backslash"));
    }

    #[test]
    fn encode_reserves_when_estimate_is_tight() {
        // `to_json` pre-sizes the mappings buffer at ~12 bytes/token. Several
        // tokens on the *same* generated line with large deltas each need more
        // than that, forcing the in-loop `reserve` (comma branch) to run.
        // Odd-indexed tokens carry no name id so the source-without-name
        // serialization branch is exercised too.
        let tokens: Vec<Token> = (0..32u32)
            .map(|i| {
                let name = if i % 2 == 0 { Some(0) } else { None };
                let source_position = (i % 2) * 536_870_912;
                Token::new(0, i * 100_000_000, source_position, source_position, Some(0), name)
            })
            .collect();
        let sm = SourceMap::new(
            None,
            vec!["n".into()],
            None,
            vec!["a.js".into()],
            vec![],
            tokens.into_boxed_slice(),
            None,
        );

        // Both encoders must round-trip the same tokens despite the realloc.
        let encoded = sm.to_json_string();
        let reparsed = SourceMap::from_json_string(&encoded).unwrap();
        assert!(sm.get_tokens().eq(reparsed.get_tokens()));
        let reparsed = SourceMap::from_json(sm.to_json()).unwrap();
        assert!(sm.get_tokens().eq(reparsed.get_tokens()));
    }

    #[test]
    fn encode_first_token_with_max_deltas() {
        // The first token of the first chunk emits no `;`/`,` delimiter, so it
        // must still trigger the per-token capacity check: its fields alone
        // can need up to 35 bytes while `to_json`'s estimate reserves ~12 per
        // token. src_line/src_col are unbounded by any array length.
        let sm = SourceMap::new(
            None,
            vec![],
            None,
            vec!["a.js".into()],
            vec![],
            vec![Token::new(0, 536_870_912, 536_870_912, 536_870_912, Some(0), None)]
                .into_boxed_slice(),
            None,
        );
        let json = sm.to_json();
        assert_eq!(json.mappings, "ggggggBAggggggBggggggB");
        let json_string = sm.to_json_string();
        let reparsed = SourceMap::from_json_string(&json_string).unwrap();
        assert!(sm.get_tokens().eq(reparsed.get_tokens()));
    }

    #[test]
    fn encode_estimates_line_breaks_of_token_chunks() {
        // A chunk writes a semicolon for every destination line it advances past its
        // starting line; sparse unminified output can have far more lines than tokens.
        let tokens: Vec<Token> =
            (0..4u32).map(|i| Token::new(i * 1_000, 0, i, 0, Some(0), None)).collect();
        let sm = SourceMap::new(
            None,
            vec![],
            None,
            vec!["a.js".into()],
            vec![],
            tokens.into_boxed_slice(),
            Some(vec![
                TokenChunk::new(0, 2, 0, 0, 0, 0, 0, 0),
                TokenChunk::new(2, 4, 1_000, 0, 1, 0, 0, 0),
            ]),
        );
        assert!(estimate_mappings_length(&sm) >= 3_000);
        let encoded = sm.to_json_string();
        let reparsed = SourceMap::from_json_string(&encoded).unwrap();
        assert!(sm.get_tokens().eq(reparsed.get_tokens()));
    }

    #[test]
    fn encode_multiline_reserves_on_line_breaks() {
        // Tokens spread across many generated lines drive the semicolon
        // (line-break) reserve branch with a tight `to_json` estimate.
        let tokens: Vec<Token> = (0..8u32)
            .map(|i| Token::new(i * 3, 50_000, 50_000, 50_000, Some(0), Some(0)))
            .collect();
        let sm = SourceMap::new(
            None,
            vec!["name".into()],
            None,
            vec!["a.js".into()],
            vec![],
            tokens.into_boxed_slice(),
            None,
        );
        let reparsed = SourceMap::from_json(sm.to_json()).unwrap();
        assert!(sm.get_tokens().eq(reparsed.get_tokens()));
    }

    #[test]
    fn encode_tokens_without_source() {
        // Generated-column-only mappings (no source id) skip the source/name
        // VLQ fields entirely — the `source_id == None` serialization branch.
        let sm = SourceMap::new(
            None,
            vec![],
            None,
            vec![],
            vec![],
            vec![Token::new(0, 0, 0, 0, None, None), Token::new(0, 4, 0, 0, None, None)]
                .into_boxed_slice(),
            None,
        );
        let encoded = sm.to_json_string();
        let reparsed = SourceMap::from_json_string(&encoded).unwrap();
        assert!(sm.get_tokens().eq(reparsed.get_tokens()));
        assert!(reparsed.get_tokens().all(|token| token.get_source_id().is_none()));
    }
}
