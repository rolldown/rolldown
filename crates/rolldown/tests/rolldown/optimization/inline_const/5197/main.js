import assert from 'node:assert'

export const glob = {
  './bar.css': () => import('./bar.mjs'),
};
export * from './export_all.mjs'

import bar from './bar.mjs';
const globEager = {
  './bar.css': bar,
};
assert.strictEqual(globEager['./bar.css'], 'bar')


