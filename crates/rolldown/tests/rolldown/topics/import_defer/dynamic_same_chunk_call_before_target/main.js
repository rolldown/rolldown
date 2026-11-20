import { p } from './c.js';
import { ns, read } from './e.js';
import { log } from './log.js';

export async function run() {
  const value = await p;
  return { same: value === ns, tag: Object.prototype.toString.call(value) };
}
export { log, read };
