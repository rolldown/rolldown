import assert from 'node:assert/strict';
import { readFileSync, statSync } from 'node:fs';

const code = readFileSync(new URL('./dist/main.js', import.meta.url), 'utf8');
assert.match(code, /^\/\/# sourceMappingURL=main\.js\.map$/m);
assert.ok(statSync(new URL('./dist/main.js.map', import.meta.url)).isFile());
