import { log } from './log.js';

log.push('a');
await 0;
throw new Error('a failed');
