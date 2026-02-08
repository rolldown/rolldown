import { strictEqual } from 'node:assert';
import { state } from './state.js';
import { getConfig } from './helpers.js';
import { TIMEOUT, MAX_RETRIES } from './shared.js';

state.config = { TIMEOUT: 5000 };

// These use the default values since config wasn't set before shared.js evaluated
strictEqual(TIMEOUT, 300000);
strictEqual(MAX_RETRIES, 3);

// But dynamic calls to getConfig see the updated config
strictEqual(getConfig('TIMEOUT', 999), 5000);

