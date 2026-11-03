import { value } from './barrel.js';

globalThis.__events.push(`main:${value}`);

export { value };
