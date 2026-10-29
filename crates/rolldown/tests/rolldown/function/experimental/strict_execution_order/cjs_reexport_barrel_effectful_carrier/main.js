import { cn } from './outer.js';

globalThis.__events.push(`main:${cn('a', 'b')}`);

export function loadRoute() {
  return import('./route.js');
}
