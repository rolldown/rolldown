import { log } from './log.js';
import defer * as d from './d.js';

// The access evaluates `m.js` synchronously. Its error throws at each access.
const read = () => {
  try {
    return d.value;
  } catch (e) {
    return e.message;
  }
};
export const first = read();
export const second = read();
export { log };
