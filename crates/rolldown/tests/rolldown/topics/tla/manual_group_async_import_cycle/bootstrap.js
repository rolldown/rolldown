import { shared } from './shared.js';
import { loadLazy } from './load-lazy.js';

const { lazy } = await loadLazy();
export const result = shared + lazy;
