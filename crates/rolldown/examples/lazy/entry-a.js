import libA from './lib-a.js';
import shared from './shared.js';
console.log('entry-a.js', libA, shared);

setTimeout(() => {
  import('./async-entry-a.js');
}, 1000 * 5);
