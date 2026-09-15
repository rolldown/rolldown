import assert from 'node:assert';
import https from 'node:https';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { getAgentCtor } = require('./dist/main.js');

assert.strictEqual(getAgentCtor(), https.Agent);
