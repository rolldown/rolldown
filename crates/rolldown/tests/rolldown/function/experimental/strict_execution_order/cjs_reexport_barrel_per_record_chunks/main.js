import { cn } from 'split-barrel';

globalThis.__events.push(`main:${cn()}`);

export const loadA = () => import('./route-a.js');
export const loadB = () => import('./route-b.js');
