import { label, mark } from './shared.js';
import { helperValue } from './lazy-dep.js';

mark('lazy-body');

export const value = helperValue;
export const sharedLabel = label;
