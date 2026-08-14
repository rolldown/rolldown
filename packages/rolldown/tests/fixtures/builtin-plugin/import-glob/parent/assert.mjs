// @ts-nocheck
import assert from 'node:assert';
import { basic, withBase, external } from './dist/main';

assert.strictEqual(Object.keys(basic).length, 2);

basic['../dir/a.js']().then((m) => {
  assert.strictEqual(m.default, 'a');
});

basic['../dir/b.js']().then((m) => {
  assert.strictEqual(m.default, 'b');
});

assert.strictEqual(Object.keys(withBase).length, 2);

withBase['./dir/a.js']().then((m) => {
  assert.strictEqual(m.default, 'a');
});

withBase['./dir/b.js']().then((m) => {
  assert.strictEqual(m.default, 'b');
});

assert.strictEqual(Object.keys(external).length, 1);

external['../basic/dir/a.js']().then((m) => {
  assert.strictEqual(m.default, 'a');
});
