// The external `import()` is lowered to `Promise.resolve().then(...)` inside this closure. The
// closure local `Promise` must not capture that `Promise`. Two reads keep the local alive.
var Promise = require('node:os');
module.exports = {
  os: [typeof Promise.platform, typeof Promise.arch],
  path: import('node:path'),
};
