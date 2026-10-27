import assert from 'node:assert/strict';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

// `c.js` and `logger.js` reach `consumer.js` only through imports that tree shaking removes. The
// chunk of `consumer.js` runs nothing observable at load time, so the runtime may stay in it. That
// keeps the chunk count of `main`: no standalone runtime chunk.
const { a } = await import('./dist/a.js');
assert.equal(a, 'log: a');

const { keys } = await import('./dist/c.js');
assert.deepEqual(keys, ['a', 'b']);

const files = readdirSync(join(import.meta.dirname, 'dist')).filter((f) => f.endsWith('.js'));
assert.ok(
  !files.some((f) => f.startsWith('rolldown-runtime')),
  `unexpected runtime chunk in: ${files.join(', ')}`,
);
