import * as ns from './dep.js';

// The property read on a namespace object is pure, but the computed key `key()`
// has a side effect and must not be tree-shaken away.
let called = false;
function key() {
  called = true;
  return 'a';
}
ns[key()];

if (!called) {
  throw new Error('side effect in the computed namespace key was dropped');
}
