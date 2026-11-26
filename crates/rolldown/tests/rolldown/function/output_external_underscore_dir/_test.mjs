import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';

writeFileSync(new URL('./dist/ext.mjs', import.meta.url), "export const marker = 'sibling';\n");

const { value } = await import('./dist/main.js');
assert.equal(value, 'intended');
