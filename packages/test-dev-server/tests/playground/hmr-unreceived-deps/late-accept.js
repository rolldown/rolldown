export const value = 'late';

globalThis.__lateAcceptValue = value;

import.meta.hot?.accept((mod) => {
  if (mod) {
    globalThis.__lateAcceptValue = mod.value;
  }
});
