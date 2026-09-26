import { shared } from './shared.js';

const { lazy } = await import('./lazy.js');
export const result = shared + lazy;
