import { DEV } from './dev.js';
import * as w from './warnings.js';
import { shared } from './shared.js';

if (DEV) w.dead();

export const a = shared();
