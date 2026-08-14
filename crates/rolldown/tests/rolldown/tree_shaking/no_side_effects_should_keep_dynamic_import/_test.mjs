import assert from 'node:assert';
import { pages, wrapper } from './dist/entry.js';

const result = await pages[0].load();
assert.strictEqual(result.message, 'Hello from page.js');
