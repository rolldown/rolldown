import { clone, nestedClone } from 'pure-boundary-barrel';

globalThis.__events.push('route');

export const value = [clone({ value: 1 }), nestedClone({ value: 2 })];
