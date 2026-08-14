import { compute } from './shared.js';

export const own = compute(3);
export const lazyLoaded = import('./lazy.js').then((m) => m.lazyValue);

// The namespace is thenable: `import('./app.js')` resolves through this
// function instead of settling on the namespace object.
export function then(resolve) {
  resolve(lazyLoaded);
}
