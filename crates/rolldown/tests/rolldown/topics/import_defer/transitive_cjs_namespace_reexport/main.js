import defer * as d from './d.js';

(globalThis.__extLog ??= []).push('main');
export const read = () => d.ns.y;
export const escape = () => d.ns;
export const loadConsumer = () => import('./consumer.js');
