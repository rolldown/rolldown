import * as shared from './shared.js';

export const b = shared;
export const lazyLog = () => import('./logger.js').then((m) => m.log('b'));
