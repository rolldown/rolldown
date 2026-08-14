import { deepStrictEqual } from 'node:assert';

export * from './dep.js';
export * from 'node:assert';

export const shadowWorks = typeof deepStrictEqual === 'function';
