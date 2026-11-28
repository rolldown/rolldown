import { baz } from './c.js';

globalThis.order.push('a sees ' + typeof baz);
export const foo = String('foo');
