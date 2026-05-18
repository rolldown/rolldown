# Chunk Hash

## Summary

Each emitted chunk's file name can contain a `[hash]` token that callers expect to be a **content-addressable** identifier: same content (across builds, machines, configurations) → same hash → same file name. This enables HTTP caching, immutable deploys, and CDN cache pinning.

The mechanism has to satisfy three invariants at once:

1. **Stability across builds.** Adding or removing an unrelated chunk must not change the hash of any chunk whose own bytes are unchanged.
2. **Sensitivity to real content.** Any change to a chunk's bytes — or to a chunk it transitively depends on — must change its hash.
3. **Uniqueness within a build.** Two distinct chunks that would resolve to the same file name must end up with different hashes.

These three pull in opposite directions, and the design described here is essentially the same as Rollup's. The non-obvious bits are: how to satisfy #1 when chunk contents quote each other by name, and how to resolve the rare conflict between #2 and #3.

## The Two-Phase Pipeline

Hash computation lives in `crates/rolldown/src/utils/chunk/finalize_chunks.rs::finalize_assets`. By the time it runs, every chunk has already been rendered into a string, and every chunk has been assigned a **preliminary filename** like `entries/main-!~{001}~.js` — the `!~{001}~` is a hash placeholder, see below.

```
[render chunks]
  ↓ chunk.content (string) + chunk.preliminary_filename (with placeholder)
[finalize_assets]
  ├─ Phase 1 (parallel):  per-chunk standalone content hash
  ├─ Phase 2 (parallel):  per-chunk final hash = own standalone + transitive deps' standalone
  ├─ Phase 3 (sequential): deconflict file names by rehashing on collision
  └─ replace placeholders in content + filename with final hashes
```

## Hash Placeholders

A hash placeholder is a fixed-shape string `!~{<index>}~` injected by `HashPlaceholderGenerator` (`crates/rolldown_utils/src/hash_placeholder.rs`) whenever rolldown needs to emit a reference to a chunk before the chunk's final hash is known. It appears in two places:

- **Inside `preliminary_filename`.** A template like `entries/[name]-[hash].js` becomes `entries/main-!~{001}~.js` once the placeholder is allocated.
- **Inside chunk content.** Anywhere an emitter writes a cross-chunk import path — `import_path_for(importee_chunk)` in `crates/rolldown_common/src/chunk/mod.rs` — the importee's `absolute_preliminary_filename` (which contains its own placeholder) is concatenated into the emitted code:
  ```js
  import { x } from './chunk-shared-!~{002}~.js';
  ```

Placeholder **shape is stable** (same length, same `!~{` / `}~` delimiters, ASCII-only). Placeholder **index is not** — it depends on the order chunks are rendered, which depends on the chunk graph, which changes when entries are added/removed.

The index is replaced with the real hash at the very end of `finalize_assets`, after all final hashes have been computed.

## Phase 1: Standalone Content Hash

```rust
let mut hasher = Xxh3::default();
visit_with_placeholders_defaulted(
  content,
  &HASH_PLACEHOLDER_LEFT_FINDER,
  |placeholder| ins_chunk_idx_by_placeholder.contains_key(placeholder),
  |bytes| hasher.update(bytes),
);
let standalone = to_url_safe_base64(hasher.digest128().to_le_bytes());
```

`visit_with_placeholders_defaulted` (in `rolldown_utils::hash_placeholder`) walks `content` and feeds bytes through `hasher.update` in order. Each `!~{xxx}~` that rolldown itself generated (the predicate looks it up in `ins_chunk_idx_by_placeholder`) is normalized to `!~{000...}~` (same shape, all-zero index) before being fed in; literals in user source code that just happen to match the placeholder shape are hashed verbatim so changes to their bytes still flow into the hash. This matches Rollup's `replacePlaceholdersWithDefaultAndGetContainedPlaceholders`, which performs the same `placeholders.has(placeholder)` check.

This is invariant #1 (stability): the chunk's own hash now depends only on its real bytes and the _shape_ of its cross-chunk references, not the transient index values.

**Streaming, not materializing.** Chunks can be megabytes. Materializing a normalized `String` per chunk would allocate roughly the bundle size's worth of throwaway buffers per build. `visit_with_placeholders_defaulted` is a visitor over `&[u8]` slices; the hasher consumes them directly. Rollup's equivalent (`replacePlaceholdersWithDefaultAndGetContainedPlaceholders`) materializes the string before hashing — rolldown deliberately doesn't.

**augmentChunkHash.** If the user's plugin supplied a hash augmentation, it gets appended to the standalone hash string and the whole thing is re-hashed (`xxhash_base64_url(hash.as_bytes())`). This matches Rollup.

## Phase 2: Final Hash

```rust
let mut hasher = Xxh3::default();
standalone_content_hashes[chunk_idx].hash(&mut hasher);
for dep_idx in transitive_dependencies[chunk_idx] {
  standalone_content_hashes[dep_idx].hash(&mut hasher);
}
let final_hash = encode_hash_with_base(hasher.digest128().to_le_bytes(), hash_base);
```

`transitive_dependencies` is computed by extracting the placeholders from each chunk's content (placeholders point to other chunks), then taking the transitive closure. Hashing every transitive dep's _standalone_ hash means:

- If chunk `B` changes, every chunk transitively depending on `B` gets a new final hash — invariant #2.
- If only an unrelated chunk's index shifts, no transitively reached chunk sees a different input — invariant #1 still holds.

The chunk's `preliminary_filename` is **deliberately not mixed into this hash**. An earlier design did (`#1141`) to guarantee uniqueness within a build, but the placeholder index inside the preliminary filename is exactly the unstable input we want to keep out. Uniqueness is enforced separately in Phase 3.

## Phase 3: Deconflict File Names

After Phase 2, two chunks with byte-identical content and identical transitive deps produce the same final hash. If their preliminary filename templates also resolve to the same string (e.g. both `entry-!~{XXX}~.js` with no `[name]` token), they would collide on disk.

`deconflict_filenames` walks chunks in deterministic order, resolves each candidate file name, and on collision **rehashes the colliding chunk** (`Xxh3(prev_hash_string)`) and tries again. Comparison is case-insensitive (HFS+/NTFS).

```rust
for chunk in chunks_in_order {
  loop {
    let candidate = resolve_filename(chunk.preliminary_filename, chunk_hash);
    if taken.insert(candidate.to_ascii_lowercase()) { break; }
    chunk_hash = rehash(chunk_hash);  // hash-of-hash
  }
}
```

This is the only sequential pass in the pipeline. It mirrors Rollup's `generateFinalHashes` (in `src/utils/renderChunks.ts`) almost line-for-line, including the case-insensitive collision set.

A regression test for this exact case lives in Rollup as `test/chunking-form/samples/hashing/deconflict-hashes`: two byte-identical entries + `entryFileNames: 'entry-[hash].js'` → two distinct file names.

In practice the collision case is rare because `experimental.attachDebugInfo` (defaulting to `Simple`) injects a `//#region <module.debug_id>` marker into rendered chunks, which differentiates content based on module path. Users who disable debug info via `experimental.attachDebugInfo: 'none'` are the ones who can trigger the collision and rely on this loop.

## Why Not Hash the Preliminary Filename Directly

Tempting alternative: mix the preliminary filename into the final hash _after normalizing its placeholder_ — this would satisfy uniqueness for chunks with different chunk names without any rehash loop.

It almost works, but fails the `deconflict-hashes` case: when two chunks have the **same chunk name** and the template is `[hash].js` (no `[name]`), their normalized preliminary filenames are byte-identical (`!~{000}~.js`), and the hash collides anyway. The rehash loop is the proper fix because it acts on the resolved file name, not on the template.

## debug_id

`ecma_meta.debug_id` (used to emit `//# debugId=...` in source maps for Sentry/etc.) is set to the same `u128` digest produced in Phase 2. This means debug IDs share the hash's stability properties — same content → same debug ID across builds, useful for sourcemap correlation. Collision-rehashed chunks naturally get a distinct debug ID too.

## Known Limitations

**Phase 3 rehashes are not propagated back into importers.** Phase 2 accumulates each transitive dep's *standalone* hash (pre-deconflict) into an importer's final hash. If Phase 3 rehashes a dep `B` to avoid a file-name collision, an importer `A` of `B` will end up emitting `B`'s post-rehash file name in its import specifier — but `A`'s own final hash was computed against `B`'s pre-rehash standalone hash, so `A`'s `[hash]` does not reflect the change. Same input + same config produces the same deconflict ordering and therefore the same emitted bytes (deterministic within a config), but two builds that differ only in something which shifts `InsChunkIdx` ordering of byte-identical chunks (e.g. user reorders entries in `input`) can produce different emitted bytes for `A` while keeping `A`'s `[hash]` unchanged. Rollup's `generateFinalHashes` exhibits the same behavior (its `contentToHash` accumulates the pre-deconflict `contentHash`, not the deconflicted final hash), so fixing this would require diverging from the reference implementation and processing chunks in topological order with importers depending on importees' post-deconflict hashes. Triggering it requires byte-identical chunks importable by something (rare) with a `[hash]`-only template (also rare).

## Files

- `crates/rolldown/src/utils/chunk/finalize_chunks.rs` — `finalize_assets`, `deconflict_filenames`, `resolve_filename`, `rehash`
- `crates/rolldown_utils/src/hash_placeholder.rs` — `HashPlaceholderGenerator`, `find_hash_placeholders`, `visit_with_placeholders_defaulted`, `replace_placeholder_with_hash`
- `crates/rolldown_common/src/chunk/types/preliminary_filename.rs` — `PreliminaryFilename` (string + owned placeholder list)
- `crates/rolldown_utils/src/xxhash.rs` — `encode_hash_with_base`, `xxhash_base64_url`

## Related

- Rollup reference implementation: [`src/utils/renderChunks.ts`](https://github.com/rollup/rollup/blob/master/src/utils/renderChunks.ts) (`transformChunksAndGenerateContentHashes`, `generateFinalHashes`)
- Issue [#9339](https://github.com/rolldown/rolldown/issues/9339) — the bug that motivated normalizing placeholders out of the standalone content hash
