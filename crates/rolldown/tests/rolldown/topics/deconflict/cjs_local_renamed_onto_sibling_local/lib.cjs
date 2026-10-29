// The local `require_dup` has the usual wrapper name of `dup.cjs`. The wrapper binding of `dup.cjs`
// is synthesized, so it takes a different name (`require_dup$2`).
const require_dup = require('./dup.cjs');

module.exports = require_dup.pair.map((item) => item.value);
