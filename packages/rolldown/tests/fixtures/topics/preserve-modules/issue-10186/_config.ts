import { defineTest } from 'rolldown-tests';
import type { Plugin } from 'rolldown';
import { expect } from 'vitest';

// Regression test for https://github.com/rolldown/rolldown/issues/10186
//
// With `preserveModules`, a module whose id is a rooted-but-drive-less path
// (`/favicon.ico`, or `\favicon.ico`) used to crash **only on Windows** with:
//   [INVALID_OPTION] Invalid substitution "/favicon" for placeholder "[name]"
//   in "entryFileNames" pattern, can be neither absolute nor relative paths.
//
// Root cause: Rust's `Path::is_absolute()` reports `/favicon` as non-absolute on
// Windows (no drive prefix), so `get_preserve_modules_chunk_name` skips the
// "make relative" branch and instead does `PathBuf::from("_virtual").join("/favicon")`,
// which discards the `_virtual` prefix and leaves the leading slash in `[name]`.
// On non-Windows the same id is absolute, so the leading slash is stripped and
// no error occurs (matching Rollup).

const ASSET_ID = '/favicon.ico';

const keepRootedId: Plugin = {
  name: 'keep-rooted-id',
  resolveId(id) {
    // Keep the leading-slash id verbatim as the module id, like Vite does for
    // public assets referenced as `/favicon.ico`.
    if (id === ASSET_ID) return id;
  },
  load(id) {
    if (id === ASSET_ID) return `export default ${JSON.stringify(ASSET_ID)}`;
  },
};

export default defineTest({
  config: {
    input: 'main.js',
    plugins: [keepRootedId],
    output: {
      preserveModules: true,
    },
  },
  afterTest: (output) => {
    const fileNames = output.output.map((chunk) => chunk.fileName);
    // The rooted id resolves relative to the filesystem root (matching Rollup
    // and the posix behavior), never leaking the leading slash into `[name]`.
    expect(fileNames).toContain('favicon.js');
    // No chunk name may be absolute/rooted (the bug leaked `/favicon`). Once a
    // rooted id joins the graph the input base collapses to the filesystem
    // root, so the entry chunk nests under this machine's absolute path (same
    // shape on every platform, but machine-specific segments) — assert the
    // shared invariant rather than an exact file list.
    for (const name of fileNames) {
      expect(name.startsWith('/')).toBe(false);
      expect(name.startsWith('\\')).toBe(false);
    }
  },
});
