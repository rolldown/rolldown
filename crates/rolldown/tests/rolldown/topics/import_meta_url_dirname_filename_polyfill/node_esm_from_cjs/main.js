import assert from 'node:assert';
import path from 'node:path';
import localBindings from './local-bindings.cjs';
import observer from './observer.cjs';
import setUnresolvedPaths from './unresolved.cjs';
import cjs from './values.cjs';

const entryDirname = import.meta.dirname;
const entryFilename = import.meta.filename;
const expectedPaths = [entryDirname, entryFilename];

assert.deepEqual(observer.readPaths(), expectedPaths);
assert.equal(observer.wasmPath, path.join(entryDirname, 'rosu_pp_js_bg.wasm'));
assert.deepEqual(localBindings, ['local dirname', 'local filename', false]);

assert.equal(cjs.dirname, import.meta.dirname);
assert.equal(cjs.filename, import.meta.filename);
assert.deepEqual(cjs.nested, [import.meta.dirname, import.meta.filename]);
assert.deepEqual(cjs.shorthand, {
  __dirname: import.meta.dirname,
  __filename: import.meta.filename,
});
assert.deepEqual(cjs.shadowed, ['local dirname', 'local filename']);

const values = Object.assign(['array dirname', 'array filename'], {
  dirname: 40,
  filename: 50,
  __dirname: 30,
  __filename: 31,
});
assert.deepEqual(cjs.setValues(values), [41, 49]);
assert.equal(
  cjs.callAssignedDirname(function () {
    return this;
  }),
  undefined,
);
assert.deepEqual(cjs.deleteDirname(), [false, 'function']);
assert.deepEqual(cjs.deleteParenthesizedDirname(), [false, 'function']);
assert.deepEqual(setUnresolvedPaths('unresolved dirname', 'unresolved filename'), [
  'unresolved dirname',
  'unresolved filename',
]);
assert.deepEqual(observer.readPaths(), expectedPaths);
assert.equal(observer.wasmPath, path.join(entryDirname, 'rosu_pp_js_bg.wasm'));
assert.equal(import.meta.dirname, entryDirname);
assert.equal(import.meta.filename, entryFilename);

// ESM source must not receive CommonJS ambient-name transforms.
assert.equal(typeof __dirname, 'undefined');
assert.equal(typeof __filename, 'undefined');
