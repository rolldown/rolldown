globalThis.__events.push('main');

export const loadA = () => import('./route-a.js');
export const loadB = () => import('./route-b.js');
