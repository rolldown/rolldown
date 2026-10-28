export const value = 'self-v0';

import.meta.hot.accept((mod) => {
  globalThis.selfAccepted = mod.value;
});
