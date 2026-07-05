import './static.js';

export function loadStatic() {
  return import('./static.js');
}

export function loadPage() {
  return import('./page.js');
}

export function loadLodash() {
  return import('./lodash.js');
}
