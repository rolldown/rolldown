import { shared } from './shared.js';
import { service } from './barrel.js';

export const value = shared + service;
export const loadLazy = () => import('./lazy.js');
export const loadSplash = () => import('./splash.js');
export const loadBarrel = () => import('./barrel.js').then((m) => m.service);
