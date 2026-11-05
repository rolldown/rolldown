import defer * as d from './d.js';

export const read = () => d.x;
export const loadE = () => import.defer('./e.js');
export const loadOther = () => import('./other.js');
