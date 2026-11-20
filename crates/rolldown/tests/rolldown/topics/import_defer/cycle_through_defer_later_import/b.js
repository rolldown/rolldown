import defer * as ns from './a.js';

globalThis.order.push('b');
export const access = () => ns.a;
