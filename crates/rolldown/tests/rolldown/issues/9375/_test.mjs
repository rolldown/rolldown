import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Mock the external `pinia` global the IIFE/UMD wrapper passes in as a factory parameter.
globalThis.Pinia = { createPinia: () => ({ __mocked: 'pinia' }) };
// `freeExports` is referenced as a top-level global by the source's environment-detection
// pattern; short-circuit it to false so the rest of the chain isn't evaluated.
globalThis.freeExports = false;

// Read the compiled bundle from this variant's `dist/`. Each `configVariants` entry runs this
// script once after `dist/` is re-populated, so both the iife and umd variants are exercised.
const bundle = readFileSync(resolve(import.meta.dirname, 'dist/index.js'), 'utf-8');

// Regression assertion: prior to the fix the inner `__commonJS` closure declared `var pinia`
// which shadowed the captured factory param `pinia`, so `(0, pinia.createPinia)()` read
// `undefined.createPinia` and threw at script-load time. After the fix the local is renamed
// to `pinia$1` and the call resolves to the mock above.
new Function(bundle)();

// Reaching this line means the bundle ran without throwing → no shadowing.
assert.ok(true);
