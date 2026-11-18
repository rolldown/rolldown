import assert from 'node:assert/strict';

const [conditionalVar, conditionalVarAlias, directEval] = await Promise.allSettled([
  import('./dist/conditional-var.js'),
  import('./dist/conditional-var-alias.js'),
  import('./dist/direct-eval.js'),
]);

assert.deepEqual(
  {
    conditionalVarThrewTypeError:
      conditionalVar.status === 'rejected' && String(conditionalVar.reason).includes('TypeError'),
    conditionalVarAliasThrewTypeError:
      conditionalVarAlias.status === 'rejected' &&
      String(conditionalVarAlias.reason).includes('TypeError'),
    directEvalCalledValueOf:
      directEval.status === 'rejected' &&
      String(directEval.reason).includes('direct eval valueOf called'),
  },
  {
    conditionalVarThrewTypeError: true,
    conditionalVarAliasThrewTypeError: true,
    directEvalCalledValueOf: true,
  },
);

const derived = await import('./dist/direct-eval-derived.js');
assert.deepEqual(derived.reads, ['b', 'b', 'b']);
