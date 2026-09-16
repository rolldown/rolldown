import { shared } from './shared.js';

const { lazy } = await import('./lazy.js');
globalThis.__tla_static_second = shared + lazy;
