import { log } from './log.js';
import { read } from './a.js';
import './d.js';

// `d.js` is also imported eagerly after `a.js`, so it evaluates at that position.
export const order = [...log];
export const value = read();
