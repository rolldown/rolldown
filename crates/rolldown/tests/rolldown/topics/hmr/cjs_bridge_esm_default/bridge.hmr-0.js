// The edit must change the rendered output — a byte-identical rebuild is
// suppressed as a no-op update and would never reach the accept handler.
module.exports = require('./esm.js');
void 'force-output-change';
