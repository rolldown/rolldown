import defer * as ns from './other.js';

globalThis.log.push('main');
export const read = () => ns.value;
