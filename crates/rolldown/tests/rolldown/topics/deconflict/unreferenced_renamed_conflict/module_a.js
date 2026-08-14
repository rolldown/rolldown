// `unused` is exported but has NO local references in this module.
// When renamed due to conflict with module_b, it becomes `unused$2`
// (not `unused$1`) because the renamer checks nested scope bindings.
export const unused = 'from-module-a';

// Parameter `unused$1` is preserved because:
// 1. The renamer skips `unused$1` when renaming top-level `unused`
// 2. The nested `unused$1` doesn't match any top-level canonical name tracked for renaming
export function test(unused$1) {
  return unused$1 + '-test';
}
