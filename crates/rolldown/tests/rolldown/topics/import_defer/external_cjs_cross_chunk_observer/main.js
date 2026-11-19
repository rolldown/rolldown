import defer * as d from './d.js';

export const lazyNamespace = () => d.b;
export const loadOther = () => import('./other.js');
