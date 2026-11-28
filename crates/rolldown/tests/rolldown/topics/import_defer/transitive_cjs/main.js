import defer * as d from './d.js';

(globalThis.__extLog ??= []).push('main');
export const read = () => [d.x, d.nsValue, d.starValue];
export const keys = () => Object.keys(d);
