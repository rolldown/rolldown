const meta = require('./meta.cjs');
globalThis.version = meta.version;
module.exports = require('./impl.cjs');
