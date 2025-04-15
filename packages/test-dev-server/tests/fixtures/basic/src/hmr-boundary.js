// import { value as depValue } from './new-dep'
// export const value = depValue;
export const value = 1;

import.meta.hot.accept((newExports) => {
  globalThis.hmrChange(newExports);
});
