// Same as entry-plain.js, plus one unrelated import that references b.js first.
import { helper } from './b.js';

export * from './c.js';
export * from './b.js';

export const usesHelper = helper;
