import { value } from './c.js';
import { ns } from './e.js';
import { log } from './log.js';

export const same = value === ns;
export const tag = Object.prototype.toString.call(value);
export const before = [...log];
export const x = value.x;
export { log };
