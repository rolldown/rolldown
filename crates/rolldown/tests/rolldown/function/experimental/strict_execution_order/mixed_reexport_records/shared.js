import { REQUIRED } from './required.js';
import { B } from './b.js';
export * from './nested.js';
export { B } from './b.js';

globalThis.events.push('shared');
export const LOCAL = REQUIRED;
