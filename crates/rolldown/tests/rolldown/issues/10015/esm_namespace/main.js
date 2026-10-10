export function f() {
  return 42;
}
export const load = () => import('./dynamic.js');
