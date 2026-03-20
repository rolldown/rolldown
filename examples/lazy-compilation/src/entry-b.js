import libB from './lib-b.js';
import shared from './shared.js';
console.log('entry-b.js', libB, shared);

setTimeout(() => {
  import('./async-entry-b.js');
}, 1000 * 6);
