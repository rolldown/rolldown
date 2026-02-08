import { strictEqual } from 'node:assert';
import { state } from './state.js';
import { getConfig } from './helpers.js';
import { TIMEOUT } from './shared.js';

state.config = { TIMEOUT: 5000 };

// If circular chunk imports caused a TDZ crash, we'd never get here.
// With the safety net (const/let->var), the early getConfig call in vendor_dep.js
// sees state as hoisted-undefined and returns the default.
strictEqual(TIMEOUT, 300000);

// After state is initialized by the entry, dynamic reads see the config.
strictEqual(getConfig('TIMEOUT', 999), 5000);

