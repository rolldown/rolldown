import { early, p, pj } from './c.js';
import { seenAfterPause } from './pause.js';
import { data, ns } from './e.js';

const tag = (value) => Object.prototype.toString.call(value);
export async function run() {
  const value = await p;
  const json = await pj;
  return {
    same: value === ns,
    tag: tag(value),
    jsonSame: json === data,
    jsonTag: tag(json),
  };
}
export { early, seenAfterPause };
