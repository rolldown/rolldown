import * as leaf from './leaf.js';
import { value } from './barrel-outer.js';

// `value` is defined in `definer.js`, reachable only through the two-level re-export barrel chain
// barrel-outer -> barrel-inner -> definer. Under strict execution order the barrel wrappers must
// initialize the definer before this read; the regression left the inner barrel's wrapper empty,
// so `value` read `undefined`.
(globalThis.__events ??= []).push('reader:' + value);

export const readerValue = value + 100;
export const leafSeen = typeof leaf;
