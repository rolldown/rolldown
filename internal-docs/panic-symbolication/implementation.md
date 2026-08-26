# Panic Symbolication — Implementation

## Summary

The published `.node` is stripped, exactly as before. The debug info it was built with is split into a side file and attached to the GitHub Release. To read a symbolicated panic backtrace, put that side file next to the `.node` and set `RUST_BACKTRACE=1`. No merge step is needed: Rust's own backtrace code already looks for a side file. This covers CI and maintainer reproduction. It does not yet resolve a backtrace that a user pastes from their terminal (see [Limits](#limits)).

## Pipeline

Everything lives in `.github/workflows/reusable-release-build.yml`, gated on a `debuginfo: true` matrix flag.

1. `Enable Debug Info` writes `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only` and `CARGO_PROFILE_RELEASE_STRIP=none` to `$GITHUB_ENV`. On `*-apple-darwin` it also sets `CARGO_PROFILE_RELEASE_SPLIT_DEBUGINFO=packed`, which makes cargo run `dsymutil`. The env overrides keep `[profile.release]` in `Cargo.toml` unchanged, so `metric.yml`, which reports the size of a plain `cargo build --release`, does not move.
2. `Build Binding` runs as before.
3. `Split Debug Info` runs `scripts/misc/split-debuginfo.mjs`. Per format:

   | target                  | debug file                                    | strip of the `.node`                                                     |
   | ----------------------- | --------------------------------------------- | ------------------------------------------------------------------------ |
   | ELF                     | `rust-objcopy --only-keep-debug`              | `rust-objcopy --strip-all`, then `--add-gnu-debuglink` to the debug file |
   | `*-apple-darwin`        | the `.dSYM` bundle cargo wrote                | `strip -x -S`                                                            |
   | `*-pc-windows-msvc`     | the `.pdb` cargo wrote                        | none — MSVC already keeps debug info out of the DLL                      |
   | `wasm32-wasip1-threads` | skipped — napi already emits a `*-debug.wasm` | none                                                                     |

   The strip commands mirror what rustc does for `strip = "symbols"` (`--strip-all` on ELF, `strip -x` on a macOS cdylib), so the shipped `.node` is the same as before.

   The output is one archive per target, `target/debuginfo/<node basename>.debuginfo.tar.zst`, holding one entry:

   - ELF: `<node basename>.debug`. The name must match the `.gnu_debuglink` record.
   - macOS: `<node basename>.dSYM/`. Any name ending in `.dSYM` works; the match is by `LC_UUID`.
   - Windows: `rolldown_binding.pdb`. The name must match the CodeView record in the DLL.

4. `Upload Debug Info Artifact` uploads it as `debuginfo-<target>`.
5. `verify-debuginfo` (one job per enabled target) downloads the binding and the archive, then runs `scripts/misc/verify-debuginfo.mjs`. That script calls `__internalForcePanic()` on the binding twice: once without the archive, asserting no source-location frames, then again after unpacking the archive next to the `.node`, asserting an `internal_force_panic` frame with a source location. On Linux it also checks with `readelf` that the `.node` has no `.debug_*` sections.
6. The `release` job in `publish-to-npm.yml` downloads `debuginfo-*` and passes the archives to `gh release create`.

## How the side file is found

`backtrace-rs` (used by `std`) locates it without any configuration:

- ELF: reads `.gnu_debuglink`, then tries `<dir>/<name>`, `<dir>/.debug/<name>`, `/usr/lib/debug/<dir>/<name>` — [`elf.rs`](https://github.com/rust-lang/backtrace-rs/blob/backtrace-v0.3.76/src/symbolize/gimli/elf.rs#L462).
- Mach-O: scans the `.node`'s directory for `*.dSYM` and matches by UUID — [`macho.rs`](https://github.com/rust-lang/backtrace-rs/blob/backtrace-v0.3.76/src/symbolize/gimli/macho.rs#L56).
- Windows: `dbghelp` searches the module's directory, then `_NT_SYMBOL_PATH`. The absolute PDB path baked in at link time points at the CI runner and is ignored.

## Reproducing a panic locally

1. Download `rolldown-binding.<platform>.node.debuginfo.tar.zst` from the release.
2. `zstd -d` it, then `tar -xf` the result into the directory that holds the `.node` (`node_modules/@rolldown/binding-<platform>/`).
3. Run the failing build with `RUST_BACKTRACE=1`.

## Enabled targets

`x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`. They cover most reports and all three debug formats. To add a target, set `debuginfo: true` on its matrix entry and add it to the `verify-debuginfo` matrix. The FreeBSD job runs in a VM and deletes `target/` before uploading; it needs its own split step.

`line-tables-only` is enough for function names and file:line and is the cheapest debug level. Fat LTO with `codegen-units = 1` makes link time and memory the cost to watch when extending.

## Limits

A user's pasted backtrace prints absolute addresses with no module base, so it cannot be resolved against the side file after the fact. The panic hook in `crates/rolldown_binding/src/lib.rs` must print module-relative addresses for that; tracked in [#10756](https://github.com/rolldown/rolldown/issues/10756). The archives produced here are what such a symbolicator would consume.

## Related

- `../devtools/implementation.md` — the other diagnostics pipeline; it does not touch panics.
