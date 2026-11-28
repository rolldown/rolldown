import { log } from './log.js';

log.push('tla start');
await Promise.resolve(0);
log.push('tla end');
