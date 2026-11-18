const lib = require('./lib.cjs');

// The entry and its dependency do not reference the wrapper's `exports` parameter.
this.answer = lib.answer;
if (this.answer !== 42 || lib.read({ answer: 0 }) !== 42 || lib.blocked !== 42) {
  throw new Error('Top-level this must reference the CommonJS exports object');
}
