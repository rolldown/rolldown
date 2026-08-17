import assert from 'node:assert';

// The `import.meta.url` polyfill references the ambient `__filename`
// (`require("url").pathToFileURL(__filename).href`). A hoisted local of the
// same name must not capture that reference: the local is still undefined
// when the polyfill runs, so `pathToFileURL` throws `ERR_INVALID_ARG_TYPE`.
// Every value is read twice so nothing is inlined past the shadowing
// declaration.
function init() {
  const url = import.meta.url;
  var __filename = 'SENTINEL';
  return [url, url, __filename, __filename];
}

const [url, , filename] = init();
assert.ok(url.startsWith('file://'));
assert.ok(!url.includes('SENTINEL'));
assert.equal(filename, 'SENTINEL');
