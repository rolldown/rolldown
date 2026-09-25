import { increment, value } from './barrel.js';
globalThis.events.push(`a:${increment()}:${value}`);
