import { shared } from './shared.js';
import { chainB } from './chain-b.js';

export const entryValue = 'entry' + shared + chainB;

export const loadCyclic = () => import('./cyclic.js');
export const loadShared = () => import('./shared.js');
export const loadChainA = () => import('./chain-a.js');
