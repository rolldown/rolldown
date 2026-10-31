// A side effect, so the unused `require()` in `lib.cjs` survives tree shaking.
globalThis.__dupBEvaluated = true;

exports.value = 'b';
