import { depValue } from './dep.js';
import './widget.js';

// dyn.js is only ever imported dynamically — the hooks assert its importer
// edge is still visible through the module-graph facade.
import('./dyn.js');

document.querySelector('.value').textContent = depValue;
window.__acceptCount = 0;

import.meta.hot.accept('./dep.js', (mod) => {
  window.__acceptCount += 1;
  document.querySelector('.value').textContent = mod.depValue;
});

// Receives the custom protocol sent from the hotUpdate hook (custom.txt scenario).
import.meta.hot.on('custom-update', (data) => {
  window.__customPayload = data;
});
