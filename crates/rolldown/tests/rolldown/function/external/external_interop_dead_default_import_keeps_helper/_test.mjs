import assert from 'node:assert';
import https from 'node:https';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

assert.strictEqual(require('./dist/dead-default-user.js').getAgentCtor(), https.Agent);
assert.strictEqual(require('./dist/live-default-user.js').getServerCtor(), https.Server);
