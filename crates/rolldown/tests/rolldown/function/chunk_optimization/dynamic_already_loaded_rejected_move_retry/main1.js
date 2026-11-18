import { shared } from './shared.js';

export const value = shared();
export const loadMiddle = () => import('./dynamic-middle.js');
export const loadLeaf = () => import('./dynamic-leaf.js');
