import { log } from './log.js';

log.push('e throws');
throw new Error('e failed');
