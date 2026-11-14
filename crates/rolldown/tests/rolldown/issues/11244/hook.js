import { memo } from 'dep';
export function hook() {
  if (true) return 1;
  return memo();
}
