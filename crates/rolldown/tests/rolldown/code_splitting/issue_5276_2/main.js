import assert from 'node:assert';
import * as ns from './imp';

assert.strictEqual(ns.imp2, 2);
assert.strictEqual(ns.imp22, 22);

const load = async () => {
  import('./imp').then((m) => {
    assert.strictEqual(m.imp22, 22);
  });
};

load();
