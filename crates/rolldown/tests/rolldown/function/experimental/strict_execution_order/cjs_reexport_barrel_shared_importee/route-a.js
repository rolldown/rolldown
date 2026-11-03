import { a } from 'shared-importee-barrel';

globalThis.__events.push('route-a');

export const value = a;
