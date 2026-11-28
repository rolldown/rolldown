import { bar } from './b.js';

globalThis.order.push('a sees ' + typeof bar);
export const foo = String('foo');
