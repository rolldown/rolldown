import { log } from './log.js';
import defer * as d from './d.js';

let result;
try {
  result = d.value;
} catch (e) {
  result = e.message;
}
log.push('i reads d: ' + result);
