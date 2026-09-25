// The local `require_dup` claims the natural wrapper name of `dup.cjs`, so that wrapper is
// deconflicted to `require_dup$2` and `require_dup` becomes a captured chunk-scope name.
const require_dup = require('./dup.cjs');

module.exports = require_dup.pair.map((item) => item.value);
