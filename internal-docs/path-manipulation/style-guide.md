# Path manipulation

Rolldown module / resolver paths are **known UTF-8**. Prefer `rolldown_std_utils` over open-coding sugar_path chains.

Implementation: `crates/rolldown_std_utils/src/path_ext.rs`. Module-id identity: [module-id/implementation.md](../module-id/implementation.md).

## Use these

| Helper                                        | For                                     |
| --------------------------------------------- | --------------------------------------- |
| `absolutize_path_buf(path)`                   | Ensure an owned path is absolute        |
| `relative_path_to_slash(target, base)`        | Relative path as `/`-separated `String` |
| `relative_path_as_js_specifier(target, base)` | Same, JS form: `.` / `./…` / `../…`     |
| `absolute_path_to_relative_slash(path, cwd)`  | Absolute → cwd-relative slash string    |
| `path_buf_to_slash(path)`                     | Owned `PathBuf` → slash `String`        |
| `PathExt::expect_to_slash`                    | Borrowed path → slash `String`          |

sugar_path 3: `relative` returns `Cow<Path>` (empty when equal); `normalize` preserves a trailing separator; slash conversion is strict by default. Explicit cwd arguments must be absolute. Keep workspace feature `cached_current_dir`.

When the destination is `ArcStr`, pass `to_slash()` directly instead of calling `into_owned()` first; `ArcStr` copies strings into its own allocation.

## Don't

```rust
target.relative(base).to_slash_lossy().into_owned()  // lossy on known UTF-8
path.to_slash().unwrap()                             // 2.x API
target.relative(base).as_path().expect_to_slash()    // skip into_slash reuse
let p: PathBuf = target.relative(base);              // not PathBuf anymore
path.to_string_lossy().replace('\\', "/")            // hand-rolled policy
```

Module **ids** stay native separators; slash form is for output / stable strings only.
