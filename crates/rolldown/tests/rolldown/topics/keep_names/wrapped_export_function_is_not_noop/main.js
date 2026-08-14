import assert from 'node:assert';
import depFromCjs from './cjs.cjs';

function dep() {}

assert.strictEqual(dep.name, 'dep');
assert.strictEqual(depFromCjs.name, 'dep');
