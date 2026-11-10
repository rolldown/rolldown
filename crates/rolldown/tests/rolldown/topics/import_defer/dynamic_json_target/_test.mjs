import assert from 'node:assert';
import { run } from './dist/main.js';

assert.deepStrictEqual(await run(), {
  sameChunk: { tag: '[object Deferred Module]', identity: true, a: 1 },
  otherChunk: { tag: '[object Deferred Module]', identity: true, b: 2 },
});
