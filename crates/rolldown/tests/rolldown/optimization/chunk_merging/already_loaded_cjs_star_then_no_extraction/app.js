import { compute } from './shared.js';

export const own = compute(21) + 1;
export const lazyLoaded = import('./lazy.js').then((m) => m.lazyValue);

// `thenable.cjs` is CommonJS, so its exports — including the callable `then` —
// are only known at runtime and never land in this entry's `resolved_exports`.
export * from './thenable.cjs';
