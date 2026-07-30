import { f0 } from './vendor.js';

console.log(f0(1));

export const lazyLoaded = import('./lazy.js').then((m) => m.fn());

// The namespace is thenable: `import('./app.js')` resolves through this function
// instead of settling on the namespace object.
export function then(resolve) {
  resolve(lazyLoaded);
}
