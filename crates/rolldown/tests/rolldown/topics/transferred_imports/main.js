import first from './first.cjs';
import { value as second } from './second.js';
import './consumer.js';
import { after } from './after.js';
globalThis.__events.push('main-anchor');
export const values = [first.value, second, after()];
