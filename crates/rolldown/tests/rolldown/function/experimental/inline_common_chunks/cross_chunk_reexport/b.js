import { increment, value } from './barrel.js';
globalThis.events.push(`b:${increment()}:${value}`);
