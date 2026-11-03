import { value as staticValue } from './new-static.js';

export const value = 'v2';
globalThis.__reload_after_failed_patch_static = staticValue;
import('./new-dynamic.js').then((mod) => {
  globalThis.__reload_after_failed_patch_dynamic = mod.value;
});
import.meta.hot.accept();
