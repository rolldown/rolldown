export async function loadFromB() {
  await import('./lazy.js').then(console.log);
}

import.meta.hot.accept();
