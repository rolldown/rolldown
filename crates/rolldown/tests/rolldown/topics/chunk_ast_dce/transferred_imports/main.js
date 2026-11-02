import first from './first.cjs';
import second from './second.cjs';
import './consumer.js';
globalThis.__transferEvents.push('transfer-main-anchor');
export const values = [first.value, second.value];
