export const loadA = () => import('./a.js');
export const loadB = () => import('./b.js');
export const loadOwn = () => import('./lib/index.js').then((lib) => lib.own);
