import { x } from './barrel.js';

globalThis.__events.push(`main:${x}`);

export { x };
