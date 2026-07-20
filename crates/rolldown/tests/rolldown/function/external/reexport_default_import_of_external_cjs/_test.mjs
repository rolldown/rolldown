const require = (await import('node:module')).createRequire(import.meta.url);
const assert = require('assert');
const https = require('node:https');
const { getAgentCtor } = require('./dist/main.js');

assert.strictEqual(getAgentCtor(), https.Agent);
