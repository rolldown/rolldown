import { cloneDeep, extraValue } from 'pure-barrel';

globalThis.__events.push(`route:${extraValue}`);

export const value = cloneDeep({ value: 1 });
