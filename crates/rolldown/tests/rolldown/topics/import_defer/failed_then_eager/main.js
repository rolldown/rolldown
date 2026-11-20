import { log, read } from './log.js';
import defer * as x from './x.js';

log.push('main: ' + read(() => x.value));
export const done = import('./j.js').then(
  () => log.push('j runs'),
  (error) => log.push('j throws: ' + error.message),
);
export { log };
