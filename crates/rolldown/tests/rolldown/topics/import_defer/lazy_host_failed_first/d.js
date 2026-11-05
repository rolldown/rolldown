import { log } from './log.js';

log.push('d');
throw new Error('d failed');
