import { EXCLUSIVE } from './exclusive.js';
import { SHARED } from './shared.js';
import cjsDep from './cjs-dep.cjs';

export function Heavy() {
  return `${EXCLUSIVE}+${SHARED}+${cjsDep.tag}`;
}
