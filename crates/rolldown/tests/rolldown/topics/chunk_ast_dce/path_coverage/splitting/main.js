import { shared } from './shared.js';

globalThis.__pathEvents.push('split-main-anchor');
export const result = import('./lazy.js').then(({ read }) => [shared, read()]);
