import { value as depValue } from './new-dep';
export const value = depValue;

import.meta.hot.accept((newExports) => {
  globalThis.hmrChange(newExports);
});
console.log('HMR boundary file changed');
