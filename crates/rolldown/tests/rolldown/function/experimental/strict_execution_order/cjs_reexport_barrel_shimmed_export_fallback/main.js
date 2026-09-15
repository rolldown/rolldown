import { cn, missing } from 'pure-barrel';

globalThis.__events.push(`main:${cn('a', 'b')}:${missing === undefined}`);

export function loadRoute() {
  return import('./route.js');
}
