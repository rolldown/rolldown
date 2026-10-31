import { value } from './value.js';
import { add } from './dep.js';

globalThis.__pathEvents.push('cross-main-anchor');
export const result = { value, sum: add(20, 22) };
