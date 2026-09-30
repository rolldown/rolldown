// Point the threaded WASI module's `malloc` / `free` exports at the heap-sync
// allocator wrappers.
//
// crates/rolldown_binding/build.rs links wasm32-wasip1-threads with
// `--wrap=malloc` / `--wrap=free`, which turns napi-build's `--export=malloc` /
// `--export=free` into `__wrap_malloc` / `__wrap_free`. @emnapi/core reads
// `exports.malloc` / `exports.free` and throws "malloc is not exported" without
// them, and it must get the locked wrappers, not dlmalloc's own entries. So,
// after the link, for `malloc` and `free`:
//
//   - drop the `__wrap_<name>` export (and an unwrapped `<name>` export, if a
//     linker ever emits one);
//   - add `<name>` for the function exported as `rolldown_heap_sync_<name>`
//     (crates/rolldown_binding/src/wasm_heap_sync.rs), which stays exported as
//     the marker scripts/wasi/check-wasi-dist-files.mjs compares against.
//
// Refuses to run twice. packages/rolldown/build-binding.ts runs it right after
// the napi build of wasm32-wasip1-threads.
// See internal-docs/wasi-shared-memory-grow/implementation.md
//
// Usage: node scripts/wasi/rename-wasm-allocator-exports.mjs <file.wasm>...

import { readFileSync, renameSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { EXPORT_KIND_FUNCTION, readExports, writeExports } from './wasm-sections.mjs';

export const ALLOCATOR_EXPORTS = ['malloc', 'free'];
export const markerExportName = (name) => `rolldown_heap_sync_${name}`;

/** Rewrite `file` in place; returns the lines describing what changed. */
export function renameWasmAllocatorExports(file) {
  const bytes = new Uint8Array(readFileSync(file));
  let exports = readExports(bytes);
  const log = [];
  for (const name of ALLOCATOR_EXPORTS) {
    const markerName = markerExportName(name);
    const marker = exports.find((entry) => entry.name === markerName);
    if (!marker || marker.kind !== EXPORT_KIND_FUNCTION) {
      throw new Error(`${file}: no function export ${markerName}; not a threaded heap-sync build`);
    }
    const current = exports.find((entry) => entry.name === name);
    if (current?.kind === EXPORT_KIND_FUNCTION && current.index === marker.index) {
      throw new Error(
        `${file}: export ${name} already points at ${markerName}; refusing to run twice`,
      );
    }
    for (const dropped of [current, exports.find((entry) => entry.name === `__wrap_${name}`)]) {
      if (!dropped) continue;
      exports = exports.filter((entry) => entry !== dropped);
      log.push(`dropped export ${dropped.name} (function ${dropped.index})`);
    }
    exports.push({ name, kind: EXPORT_KIND_FUNCTION, index: marker.index });
    log.push(`added export ${name} -> function ${marker.index} (${markerName})`);
  }
  const renamed = writeExports(bytes, exports);
  if (!WebAssembly.validate(renamed)) {
    throw new Error(`${file}: the renamed module does not validate; left unchanged`);
  }
  const temporary = `${file}.rename-tmp`;
  writeFileSync(temporary, renamed);
  renameSync(temporary, file);
  return log;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const files = process.argv.slice(2);
  if (files.length === 0) {
    console.error('Usage: node scripts/wasi/rename-wasm-allocator-exports.mjs <file.wasm>...');
    process.exit(2);
  }
  try {
    for (const file of files) {
      for (const line of renameWasmAllocatorExports(file)) console.log(`${file}: ${line}`);
    }
  } catch (error) {
    console.error(error.message);
    process.exit(1);
  }
}
