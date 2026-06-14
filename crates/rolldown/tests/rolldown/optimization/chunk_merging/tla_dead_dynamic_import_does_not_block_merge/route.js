import assert from 'node:assert';
import { main } from './main.js';

export const route = `${main}:route`;

assert.strictEqual(route, 'main:shared:route');
