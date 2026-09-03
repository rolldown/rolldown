export async function loadFromC() {
  console.log('c no longer loads lazy');
}

import.meta.hot.accept();
