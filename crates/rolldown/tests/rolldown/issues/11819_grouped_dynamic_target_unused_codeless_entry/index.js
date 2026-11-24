import assert from 'node:assert/strict';

const ns = await import('./target.js');
assert.equal(ns.message(), 'ready');
export const lazy = () => import('./barrel.js');
