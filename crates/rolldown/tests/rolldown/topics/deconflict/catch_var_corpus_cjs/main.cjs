const assert = require('node:assert/strict');
const dep = require('./dep.cjs');

var x = 'main-x';

// Top-level linked catches in the unwrapped entry: `y` and `z` must step around the inner ones in
// dep.cjs, and the Annex B functions must follow the `var`, unless a lexical binding blocks them.
var annex = [];
try {
  throw 0;
} catch (y) {
  var y = 1;
  {
    function y() {
      return 'fy';
    }
    annex.push(y());
  }
  annex.push(typeof y);
}
annex.push(typeof y);
try {
  throw 0;
} catch (z) {
  var z = 1;
  {
    let z = 'lexical';
    z += '!';
    {
      function z() {}
    }
    annex.push(z);
  }
  annex.push(z);
}
annex.push(typeof z);
assert.deepStrictEqual(dep.fixed(), [2, 'number', 'function']);
assert.deepStrictEqual(dep.fixedModuleLevel(), ['local', undefined]);
assert.deepStrictEqual(dep.nestedAnnexB(), ['set', 'outer', 'function']);
assert.deepStrictEqual(dep.esm.esmLog, ['esm-c', 'other-x', 'esm-x']);
assert.deepStrictEqual(dep.esm.innerEsm(), ['inner', 'other-x']);
assert.strictEqual(x, 'main-x');
assert.deepStrictEqual(dep.log, ['fx', 'number', 'function', 'fp', 'string', 'function']);
assert.deepStrictEqual(dep.inner(), [undefined, undefined]);
assert.deepStrictEqual(annex, ['fy', 'number', 'function', 'lexical!', 1, 'undefined']);
dep.lazy().then((value) => assert.strictEqual(value, 'other-x'));
