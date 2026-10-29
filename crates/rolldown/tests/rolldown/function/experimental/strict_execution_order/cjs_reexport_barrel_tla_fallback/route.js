import { cloneDeep } from 'pure-barrel';

globalThis.__events.push('route');

export const value = cloneDeep({ value: 1 });
