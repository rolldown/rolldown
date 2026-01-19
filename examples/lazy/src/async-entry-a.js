import asyncLibA from './async-lib-a.js';
import './async-lib-shared.js';

console.log('async-entry-a.js', asyncLibA);
document.getElementById('root').innerHTML += '[async-entry-a.js] loaded\n';

import './inlined.js';
setTimeout(async () => {
  const exports = await import('./inlined.js');
  console.log('Loaded inlined.js:', exports);
}, 1000 * 1);
