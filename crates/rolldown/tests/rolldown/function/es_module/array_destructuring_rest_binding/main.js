import assert from 'node:assert';
import { stages } from './lib';
const values = Object.values(stages);
const [first, ...rest] = values;
if (!first) throw new Error('empty');
export const all = [first, ...rest];
assert.deepStrictEqual(all, ['1', '2', '3']);
