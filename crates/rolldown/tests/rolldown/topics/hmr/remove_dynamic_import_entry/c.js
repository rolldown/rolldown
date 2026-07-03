export async function loadFromC() {
  await import('./lazy.js').then(console.log);
}

import.meta.hot.accept();
