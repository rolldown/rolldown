import { cloneDeep, extLibValue } from 'pure-barrel';

globalThis.__events.push(`route:${extLibValue}`);

export const value = cloneDeep({ value: 1 });
