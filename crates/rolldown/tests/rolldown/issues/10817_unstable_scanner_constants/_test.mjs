import assert from 'node:assert/strict';

const [conditionalVar, directEval] = await Promise.allSettled([
  import('./dist/conditional-var.js'),
  import('./dist/direct-eval.js'),
]);

assert.deepEqual(
  {
    conditionalVarThrewTypeError:
      conditionalVar.status === 'rejected' && String(conditionalVar.reason).includes('TypeError'),
    directEvalCalledValueOf:
      directEval.status === 'rejected' &&
      String(directEval.reason).includes('direct eval valueOf called'),
  },
  {
    conditionalVarThrewTypeError: true,
    directEvalCalledValueOf: true,
  },
);
