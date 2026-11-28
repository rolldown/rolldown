import './setup.js';
import './external.js';

export const loadRoute = () => import('./route.js');
export const loadSplash = () => import('./splash.js').then((m) => m.v);
export const loadBarrel = () => import('./barrel.js').then((m) => m.service);
