import assert from 'node:assert';

// A module may declare its own `require` binding, which emscripten does in its
// generated glue. The polyfill for `import.meta.url` must not resolve to that
// binding. The local has to be read more than once, otherwise it is inlined
// into the call expression and no shadowing survives.
function init() {
  var require = createRequire(import.meta.url);

  const path = require('node:path');
  const os = require('node:os');

  return path.sep + os.EOL;
}

import { createRequire } from 'node:module';

assert.equal(typeof init(), 'string');
