import { compute } from './shared.js';

export const own = compute(21) + 1;
export const lazyLoaded = import('./lazy.js').then((m) => m.lazyValue);

// `thenable.js` is bundled, so its `then` is statically known: it becomes part
// of this entry's resolved exports and makes the namespace thenable.
export * from './thenable.js';
