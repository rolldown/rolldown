export async function loadFromB() {
  console.log('b no longer loads lazy');
}

import.meta.hot.accept();
