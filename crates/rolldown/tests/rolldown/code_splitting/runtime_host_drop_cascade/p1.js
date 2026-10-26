export const loadP3 = () => import('./p3.js');
export const lazyLog = () => import('./logger.js').then((m) => m.log('p1'));
