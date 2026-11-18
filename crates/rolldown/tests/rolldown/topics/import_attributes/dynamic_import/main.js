import assert from 'node:assert';
import { instance } from './dep.js';

const tagged = await import('./dep.js', { with: { kind: 'x' } });
const same = await import('./dep.js', { with: { kind: 'x' } });
const plain = await import('./dep.js');

assert.strictEqual(plain.instance, instance);
assert.notStrictEqual(tagged.instance, instance);
assert.strictEqual(same.instance, tagged.instance);
