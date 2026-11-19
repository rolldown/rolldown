import { log } from './log.js';
import defer * as root from './root.js';

export function run() {
  return [root.value, root.readT(), ...log];
}
