import { log } from './log.js';

log.push('a start');
await 0;
throw new Error('a failed');
