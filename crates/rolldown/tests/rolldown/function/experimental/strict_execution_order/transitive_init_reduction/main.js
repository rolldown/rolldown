// main imports a, b, c directly. Since a→b→c, only init_a should appear.
import { value as va } from './a.js';
import { value as vb } from './b.js';
import { value as vc } from './c.js';
import assert from 'node:assert';

assert.equal(va, 'a:b:c');
assert.equal(vb, 'b:c');
assert.equal(vc, 'c');
