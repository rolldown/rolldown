export const value = 1;

import.meta.hot.accept((newExports) => {
  globalThis.hmrChange(newExports);
});
