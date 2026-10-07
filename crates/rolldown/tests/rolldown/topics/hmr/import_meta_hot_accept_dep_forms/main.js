import assert from 'node:assert';
import './one_arg/parent.js';
import './three_args/parent.js';
import './template/parent.js';
import './template_array/parent.js';
import './falsy_deps/parent.js';
import './array_one_arg/parent.js';

process.on('beforeExit', (code) => {
  if (code !== 0) return;
  // Each accepting parent is a boundary: its dep re-runs, the parent does not.
  assert.strictEqual(globalThis.one_argChildRuns, 2);
  assert.strictEqual(globalThis.one_argParentRuns, 1);

  assert.strictEqual(globalThis.three_argsChildRuns, 2);
  assert.strictEqual(globalThis.three_argsParentRuns, 1);
  assert.strictEqual(globalThis.three_argsSaw, 1);

  assert.strictEqual(globalThis.templateChildRuns, 2);
  assert.strictEqual(globalThis.templateParentRuns, 1);
  assert.strictEqual(globalThis.templateSaw, 1);

  assert.strictEqual(globalThis.template_array_aRuns, 2);
  assert.strictEqual(globalThis.template_array_bRuns, 2);
  assert.strictEqual(globalThis.template_arrayParentRuns, 1);
  assert.deepStrictEqual(globalThis.template_arraySaw, [1, 1]);

  assert.strictEqual(globalThis.array_one_argChildRuns, 2);
  assert.strictEqual(globalThis.array_one_argParentRuns, 1);

  assert.strictEqual(globalThis.falsy_depsSelfRuns, 2);
  assert.strictEqual(globalThis.falsy_depsParentRuns, 1);
  assert.strictEqual(globalThis.falsy_depsCallbackRan, undefined);
});
