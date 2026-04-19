import { normalize } from './normalize.js';

// Mimics bindingify-input-options.ts:
// - `preserveEntrySignatures` is imported by plugin-context (used by worker entry)
// - `bindingifyInputOptions` uses `normalize` (only reached by main entry)
export function preserveEntrySignatures(value) {
  return value;
}

export function bindingifyInputOptions(opts) {
  return normalize(opts);
}
