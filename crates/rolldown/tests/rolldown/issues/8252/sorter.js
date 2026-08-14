import { sort } from './sorting';
import { resolveJsFrom } from './resolve';

export function createSorter() {
  return { sort, resolveJsFrom };
}
