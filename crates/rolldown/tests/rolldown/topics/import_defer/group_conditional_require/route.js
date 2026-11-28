import { offset } from './helper.js';

export function read(flag) {
  return flag ? require('./lazy.js').x + offset : offset;
}
