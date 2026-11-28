import defer * as dep from './dep.js';
import { log } from './log.js';

export const before = [...log];
export const value = dep.value;
export const after = [...log];
