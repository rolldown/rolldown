export const value = 'v2';
globalThis.__no_reload_after_full_build_runs =
  (globalThis.__no_reload_after_full_build_runs ?? 0) + 1;
import.meta.hot.accept();
