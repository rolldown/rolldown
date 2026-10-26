import assert from 'node:assert';
import data from 'data:application/json,{"value":1}' with { type: 'json' };
import value from 'data:text/javascript,export default 2' with { kind: 'x' };

assert.strictEqual(data.value, 1);
assert.strictEqual(value, 2);
