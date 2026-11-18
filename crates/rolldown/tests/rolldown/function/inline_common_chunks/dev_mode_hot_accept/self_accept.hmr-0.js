export const value = 'self-v1';

import.meta.hot.accept((mod) => {
  globalThis.selfAccepted = mod.value;
});
