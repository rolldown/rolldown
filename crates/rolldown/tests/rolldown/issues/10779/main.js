import assert from 'node:assert/strict';

globalThis.window = {};

function load(value = typeof window !== 'undefined' ? 'browser' : 'server') {
  var window;
  return value;
}

assert.strictEqual(load(), 'server', 'define should replace typeof window in a default parameter');
