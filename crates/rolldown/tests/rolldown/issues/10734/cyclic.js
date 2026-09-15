// Closes a cycle: a lazily imported module that imports back from the dynamic entry.
import { chainB } from './chain-b.js';
import { entryValue } from './service.js';

export const cyclic = 'cyclic' + chainB + entryValue;
