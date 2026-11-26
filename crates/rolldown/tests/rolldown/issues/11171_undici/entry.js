import assert from 'node:assert/strict';

const { Agent, default: undici } = await import('./re.js');
assert.equal(typeof Agent, 'function');
assert.equal(Agent, undici.Agent);
assert.ok(new Agent() instanceof Agent);
