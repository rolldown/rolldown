import DEV from './dev.js';
import { dead } from './warnings.js';
import { shared } from './shared.js';

if (DEV) dead();
DEV && dead();
!DEV || dead();
DEV ? dead() : 0;

export const named = shared();
