import { log } from './log.js';
import defer * as root from './root.js';

export function run() {
  const read = root.read;
  const before = [...log];
  const value = read();
  return { before, value, after: log };
}
