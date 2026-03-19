import libA from './lib-a.js';
import shared from './shared.js';
console.log('entry-a.js', libA, shared);
document.getElementById('root').innerHTML += 'entry-a.js loaded\n';

setTimeout(async () => {
  const exports = await import('./async-entry-a.js');
  console.log('Imported async-entry-a.js:', exports);
}, 1000 * 5);

setTimeout(async () => {
  const exports = await import('./async-entry-b.js');
  console.log('Imported async-entry-b.js:', exports);
}, 1000 * 6);
