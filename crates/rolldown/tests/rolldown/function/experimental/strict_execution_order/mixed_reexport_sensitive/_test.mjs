import assert from 'node:assert/strict';
import { local, load } from './dist/main.js';

assert.equal(local, 'local');
const [snapshot, pure] = await load();
assert.equal(pure, 'pure');
assert.equal(snapshot, 0, 'SNAPSHOT must be read before main calls bump()');
