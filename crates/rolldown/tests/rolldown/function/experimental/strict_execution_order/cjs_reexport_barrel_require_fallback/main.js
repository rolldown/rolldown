import { cn } from 'required-barrel';

globalThis.__events.push(`main:${cn()}`);

export const loadRoute = () => import('./route.cjs');
