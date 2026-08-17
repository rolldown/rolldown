import { compute } from './shared.js';

const own = compute(21) + 1;
export { own as app_exports };
export const done = import('./lazy.js').then((m) => m.lazyValue);
