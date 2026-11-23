import { f } from './barrel.js';

globalThis.calls.push(`main done: ${typeof f}`);
