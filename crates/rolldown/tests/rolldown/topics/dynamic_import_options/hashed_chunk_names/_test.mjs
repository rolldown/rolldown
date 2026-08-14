import assert from 'node:assert';
import fs from 'node:fs';

const dist = new URL('./dist/', import.meta.url);
const chunks = fs.readdirSync(dist).filter((name) => name.startsWith('target-'));
assert.strictEqual(chunks.length, 1, `expected one hashed target chunk, got ${chunks}`);
assert.strictEqual(fs.existsSync(new URL('./target.js', dist)), false);

const { loaded } = await import('./dist/main.js');
assert.strictEqual(await loaded, 42);
