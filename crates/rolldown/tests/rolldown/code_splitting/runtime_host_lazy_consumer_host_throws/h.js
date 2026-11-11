import * as hn from './hn.js';
console.log('h', Object.keys(hn).join());
globalThis.lazy = import('./d.js').then(
  () => 'loaded',
  (error) => error.message,
);
throw new Error('host failed');
