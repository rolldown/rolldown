import assert from 'node:assert';

const server = await import('./dist/server.js');

// The server entry's default and `routes` must work as before.
assert.strictEqual(typeof server.default, 'function');
assert.strictEqual(server.default(), 'server-entry-default Symbol(client-only)');

// The dynamic route imports client-only.js. After chunk merging that target
// became server.js — the bundler must rewrite the dynamic import so the
// returned namespace is client-only's (not server's). `r.default || r` should
// resolve to client-only's default export string, not to server's default
// (the `entry` function).
const route = await server.routes['/']();
const value = await route.default.loadString();
assert.strictEqual(value, 'default export');
