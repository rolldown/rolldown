import { log } from './log.js';
import defer * as d from './d.js';

log.push('i');
let result;
try {
  d.value;
  result = 'ok';
} catch (e) {
  result = e.constructor.name;
}
log.push('i reads d: ' + result);
