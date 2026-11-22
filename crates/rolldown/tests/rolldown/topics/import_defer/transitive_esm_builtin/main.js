import defer * as d from './d.js';

(globalThis.__extLog ??= []).push('main');
export const read = () => d.x;
