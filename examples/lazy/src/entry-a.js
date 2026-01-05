import libA from './lib-a.js';
import shared from './shared.js';
console.log('entry-a.js', libA, shared);
document.getElementById('root').innerHTML += 'entry-a.js loaded\n';

async function lazyMagic(proxyModule) {
  const exports = proxyModule['rolldown:exports'];
  if (exports) {
    return await exports;
  }
  return proxyModule;
}

setTimeout(async () => {
  const exports = await import('./async-entry-a.js').then(lazyMagic);
  console.log('Imported async-entry-a.js:', exports);
}, 1000 * 5);
