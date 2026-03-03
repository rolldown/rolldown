import { strictEqual } from 'node:assert';

const { state } = require('./state.js');
const { getConfig } = require('./helpers.js');
const { TIMEOUT, MAX_RETRIES } = require('./shared.js');

state.config = { TIMEOUT: 5000 };

// CJS modules are wrapped with lazy init, so TIMEOUT/MAX_RETRIES use defaults
strictEqual(TIMEOUT, 300000);
strictEqual(MAX_RETRIES, 3);

// Dynamic calls see the updated config
strictEqual(getConfig('TIMEOUT', 999), 5000);
