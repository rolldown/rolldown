import { readerValue } from './reader.js';

(globalThis.__events ??= []).push('reader-host:' + readerValue);

// A second dynamic import of `leaf`, which `reader` also statically namespace-imports. This mirrors
// the shape the fuzzer minimized to and keeps `leaf` in the dynamic chunk next to the reader.
export function loadLeaf() {
  return import('./leaf.js');
}
