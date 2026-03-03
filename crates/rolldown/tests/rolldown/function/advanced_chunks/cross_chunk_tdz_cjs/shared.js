// CJS module that would create a circular chunk dep if split.
// Since CJS modules are wrapped, this split is safe.
const { getConfig } = require('./helpers.js');

const TIMEOUT = getConfig('TIMEOUT', 300000);
const MAX_RETRIES = getConfig('MAX_RETRIES', 3);

module.exports = { TIMEOUT, MAX_RETRIES };
