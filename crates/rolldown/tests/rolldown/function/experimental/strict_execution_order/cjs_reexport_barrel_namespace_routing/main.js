import * as ns from 'namespace-barrel';

globalThis.__events.push(`main:${ns.cn()}`);

export const loadStatic = () => import('./route-static.js');
export const loadOpaque = () => import('./route-opaque.js');
