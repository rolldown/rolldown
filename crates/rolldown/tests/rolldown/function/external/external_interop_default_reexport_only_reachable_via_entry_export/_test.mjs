import assert from 'node:assert';
import https from 'node:https';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

assert.strictEqual(require('./dist/entry-reexport.js').d.Agent, https.Agent);
assert.strictEqual(require('./dist/entry-reader.js').getServerCtor(), https.Server);
