import { b } from 'shared-importee-barrel';

globalThis.__events.push('route-b');

export const value = b;
