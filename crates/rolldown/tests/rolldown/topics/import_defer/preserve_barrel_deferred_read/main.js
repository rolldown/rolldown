import { ns, eager } from './barrel.js';
globalThis.events.push('main');
export const result = [eager, ns.x];
