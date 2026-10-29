import { mark } from './shared.js';

mark('main-body');

export const ready = Promise.all([import('./lazy.js'), import('./lazy.js')]).then(
  ([first, second]) => {
    mark(`lazy-resolved:${first.value}:${first.sharedLabel}`);
    mark(`same-namespace:${first === second}`);
  },
);

globalThis.ready = ready;

// Reached synchronously, so the wrapped dynamic target must still be uninitialized here.
mark(`lazy-pending:${!globalThis.log.includes('lazy-body')}`);
