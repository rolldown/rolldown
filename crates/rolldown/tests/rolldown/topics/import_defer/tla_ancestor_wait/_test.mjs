import assert from 'node:assert';
import { log } from './dist/main.js';

// Rolldown runs the TLA siblings `a.js` and `i.js` in sequence. Native ESM runs `i.js` while `a.js` waits.
// `main.js` waits for `a.js` in its import list, so `main.js` is still evaluating when `i.js` reads `d.js`.
// The read throws `TypeError`, and the build does not deadlock.
assert.deepStrictEqual(log, ['a start', 'a end', 'i', 'i reads d: TypeError', 'main']);
