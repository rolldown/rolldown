import assert from 'node:assert';
import { shared } from './shared.js';

export const main = `main:${shared}`;

async function unused() {
  return import('./route.js');
}

await Promise.resolve();

assert.strictEqual(main, 'main:shared');
