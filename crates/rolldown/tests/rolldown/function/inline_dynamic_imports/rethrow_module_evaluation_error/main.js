import assert from 'node:assert';

const NOT_THROWN = Symbol('not thrown');

(async () => {
  // #4461: an error thrown while evaluating the module must be rethrown on every import,
  // not just the first one. The same error instance is expected each time.
  const errors = [];
  for (let i = 0; i < 2; i++) {
    let thrown = NOT_THROWN;
    try {
      await import('./throws_error.js');
    } catch (e) {
      thrown = e;
    }
    errors.push(thrown);
  }
  assert.ok(errors[0] instanceof Error, 'every import of throws_error.js should throw');
  assert.strictEqual(errors[0].message, 'evaluation error');
  assert.strictEqual(errors[1], errors[0], 'second import should rethrow the same error');

  // #4467: even a falsy thrown value (e.g. `throw null`) must be rethrown on every import.
  const nulls = [];
  for (let i = 0; i < 2; i++) {
    let thrown = NOT_THROWN;
    try {
      await import('./throws_null.js');
    } catch (e) {
      thrown = e;
    }
    nulls.push(thrown);
  }
  assert.strictEqual(nulls[0], null, 'every import of throws_null.js should throw null');
  assert.strictEqual(nulls[1], null, 'every import of throws_null.js should throw null');
})();
