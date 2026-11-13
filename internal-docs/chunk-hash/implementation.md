# Chunk Hash

## Summary

Each emitted chunk's file name can contain a `[hash]` token that callers expect to be a **content-addressable** identifier: same content (across builds, machines, configurations) → same hash → same file name. This enables HTTP caching, immutable deploys, and CDN cache pinning.

The mechanism has to satisfy three invariants at once:

1. **Stability across builds.** Adding or removing an unrelated chunk must not change the hash of any chunk whose own bytes are unchanged.
2. **Sensitivity to real content.** Any change to a chunk's bytes — or to a chunk it transitively depends on — must change its hash.
3. **Uniqueness within a build.** Two distinct chunks that would resolve to the same file name must end up with different hashes.

These three pull in opposite directions, and the design described here is essentially the same as Rollup's. The non-obvious bits are: how to satisfy #1 when chunk contents quote each other by name, and how to resolve the rare conflict between #2 and #3.

## The Pipeline

Hash computation lives in `crates/rolldown/src/utils/chunk/finalize_chunks.rs::finalize_assets`. By the time it runs, every chunk has already been rendered into a string, and every chunk has been assigned a **preliminary filename** like `entries/main-!~{001}~.js` — the `!~{001}~` is a hash placeholder, see below.

```
[render chunks]
  ↓ chunk.content (string) + chunk.preliminary_filename (with placeholder)
[finalize_assets]
  ├─ prepare sourcemaps with preliminary chunk names
  ├─ Phase 1 (parallel):  per-chunk standalone content hash
  ├─ Phase 2 (parallel):  per-chunk base hash = own standalone + transitive deps' standalone
  ├─ Phase 3 (sequential): final hashes, dependencies first: fold in renamed deps, rehash on collision
  ├─ replace placeholders in content + filename with final hashes
  └─ resolve sourcemap chunk names, hash their final content, then emit them
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

**Sourcemap comments.** `hash_sourcemap_comment` includes the comment emitted after the rendered code. External map filenames are normalized before URL encoding, using the same URL construction as emission. Inline maps contribute their prepared JSON before Base64 encoding. Normalizing after encoding would miss placeholders and make the hash depend on unrelated chunks' indices. Hidden maps contribute nothing unless debug IDs add a comment.

For an external map filename containing `[hash]`, the comment's hash input uses a hash of the prepared map with chunk placeholders normalized. The emitted map's hash still uses its final JSON, including the final chunk name. This breaks the circular dependency (chunk hash → map's `file` → map hash → chunk comment) while making changes to referenced map content invalidate the chunk and its importers. Map content does not affect the chunk hash when the comment uses a fixed map filename.

## Phase 2: Base Hash

```rust
let mut hasher = Xxh3::default();
standalone_content_hashes[chunk_idx].hash(&mut hasher);
for dep_idx in transitive_dependencies[chunk_idx] {
  standalone_content_hashes[dep_idx].hash(&mut hasher);
}
let base_hash = encode_hash_with_base(hasher.digest128().to_le_bytes(), hash_base);
```

`transitive_dependencies` is computed by extracting the placeholders from each chunk's content (placeholders point to other chunks), then taking the transitive closure. Hashing every transitive dep's _standalone_ hash means:

- If chunk `B` changes, every chunk transitively depending on `B` gets a new final hash — invariant #2.
- If only an unrelated chunk's index shifts, no transitively reached chunk sees a different input — invariant #1 still holds.

The chunk's `preliminary_filename` is **deliberately not mixed into this hash**. An earlier design did (`#1141`) to guarantee uniqueness within a build, but the placeholder index inside the preliminary filename is exactly the unstable input we want to keep out. Uniqueness is enforced separately in Phase 3.

## Phase 3: Final Hashes

After Phase 2, two chunks with byte-identical content and identical transitive deps produce the same base hash. If their preliminary filename templates also resolve to the same string (e.g. both `entry-!~{XXX}~.js` with no `[name]` token), they would collide on disk. Rehashing one of them renames it, and every chunk that imports it now quotes a name its base hash never saw, so their names must change too ([#11694](https://github.com/rolldown/rolldown/issues/11694)).

`assign_final_hashes` settles both at once:

1. File names without a hash are claimed first; they cannot move out of the way.
2. Chunks are grouped into import cycles (strongly connected components, via petgraph's iterative `kosaraju_scc`; `tarjan_scc` recurses and overflows on deep chains). Components are processed dependencies first (Kahn's algorithm); among ready components the one with the lowest chunk index goes first.
3. For each component:
   - If any chunk it imports from outside was renamed (final hash ≠ base hash), each member's hash becomes `Xxh3(base hash, final hashes of its transitive deps outside the component)`. Otherwise the final hash is the base hash, so builds without collisions keep exactly the Phase 2 hashes.
   - Resolve every member's file name. On a collision (case-insensitive, HFS+/NTFS), **rehash every member** of the component (`Xxh3(prev_hash_string)`, plus the member's rank in a multi-chunk component) and try again. Members of a cycle quote each other's names, so renaming one renames them all; the rank keeps byte-identical members of one cycle from colliding forever.

Why the result is sound: a member's final hash commits to the contents of its whole component, the final hashes of everything it imports outside it, and the number of rehash rounds. Those determine every name its content quotes, so one final name always means one set of emitted bytes.

For a single chunk with no renamed dependency this is the same rehash-on-collision loop as Rollup's `generateFinalHashes` (in `src/utils/renderChunks.ts`), and the lowest index still wins among unrelated colliding chunks. Propagating renames into importers is a deliberate divergence: Rollup accumulates the pre-deconflict hash, so an importer can keep its name while its content changes.

A regression test for the plain collision case lives in Rollup as `test/chunking-form/samples/hashing/deconflict-hashes`: two byte-identical entries + `entryFileNames: 'entry-[hash].js'` → two distinct file names. The propagation case is covered by `packages/rolldown/tests/behaviors/hash-stability.test.ts` and the unit tests in `finalize_chunks.rs`.

In practice the collision case is rare because `experimental.attachDebugInfo` (defaulting to `Simple`) injects a `// <module.debug_id>` marker into rendered chunks, which differentiates content based on module path. Collisions need that marker gone, e.g. `experimental.attachDebugInfo: 'none'` or `minify: true`.

## Sourcemap File Hashes

`output.sourcemapFileNames` has its own `[hash]` placeholder. It is resolved after chunk hashes because the final sourcemap contains the final chunk file name in its `file` field.

The sourcemap sequence is:

1. Prepare each sourcemap using preliminary chunk names: set `file`, apply `sourcemapExcludeSources`, `sourcemapIgnoreList`, and `sourcemapPathTransform`, and make source paths relative to the chunk directory.
2. Resolve chunk hashes and final chunk file names, then update the map's `file` and substitute chunk placeholders in source paths.
3. If the sourcemap file-name pattern contains `[hash]`, hash the prepared map's serialized JSON.
4. Resolve the sourcemap file name, then add debug IDs and emit the inline comment or separate asset.

Preparation and emission are separate so user callbacks run only once while every content-changing map option contributes to `[hash]`. Debug IDs are deliberately added after the sourcemap hash, matching Rollup. The path passed to sourcemap callbacks is the default `<preliminary-chunk-file>.map` path (possibly containing hash placeholders), which avoids circular dependencies through either a chunk hash or a custom sourcemap hash. A callback that incorporates this path into a source path has its chunk placeholders resolved before emission.

## Why Not Hash the Preliminary Filename Directly

Tempting alternative: mix the preliminary filename into the final hash _after normalizing its placeholder_ — this would satisfy uniqueness for chunks with different chunk names without any rehash loop.

It almost works, but fails the `deconflict-hashes` case: when two chunks have the **same chunk name** and the template is `[hash].js` (no `[name]`), their normalized preliminary filenames are byte-identical (`!~{000}~.js`), and the hash collides anyway. The rehash loop is the proper fix because it acts on the resolved file name, not on the template.

## debug_id

`ecma_meta.debug_id` (used to emit `//# debugId=...` in source maps for Sentry/etc.) is set to the same `u128` digest as the final hash from Phase 3. This means debug IDs share the hash's stability properties — same content → same debug ID across builds, useful for sourcemap correlation. Rehashed chunks and their importers naturally get a distinct debug ID too.

## Files

- `crates/rolldown/src/utils/chunk/finalize_chunks.rs` — `finalize_assets`, `assign_final_hashes`, `resolve_filename`, `rehash`
- `crates/rolldown_utils/src/hash_placeholder.rs` — `HashPlaceholderGenerator`, `find_hash_placeholders`, `visit_with_placeholders_defaulted`, `replace_placeholder_with_hash`
- `crates/rolldown_common/src/chunk/types/preliminary_filename.rs` — `PreliminaryFilename` (string + owned placeholder list)
- `crates/rolldown_utils/src/xxhash.rs` — `encode_hash_with_base`, `xxhash_base64_url`

## Related

- Rollup reference implementation: [`src/utils/renderChunks.ts`](https://github.com/rollup/rollup/blob/master/src/utils/renderChunks.ts) (`transformChunksAndGenerateContentHashes`, `generateFinalHashes`)
- Issue [#9339](https://github.com/rolldown/rolldown/issues/9339) — the bug that motivated normalizing placeholders out of the standalone content hash
- Issue [#11694](https://github.com/rolldown/rolldown/issues/11694) — a chunk renamed by collision handling left its importers' names unchanged
