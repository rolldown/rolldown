export const value = 'mod-a';

import { value as _value } from './mod-b.js';

export const msg = `mod-a -> ${_value}`;
