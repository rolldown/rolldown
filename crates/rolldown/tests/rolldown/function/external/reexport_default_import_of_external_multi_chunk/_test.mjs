import assert from 'node:assert';
import https from 'node:https';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

assert.strictEqual(require('./dist/default-user.js').getAgentCtor(), https.Agent);
assert.strictEqual(require('./dist/another-default-user.js').getServerCtor(), https.Server);
assert.strictEqual(require('./dist/named-user.js').getAgentCtorFromNamed(), https.Agent);
