// `dep.cjs` makes the runtime a dependency of this entry. So the runtime has its own atom, and the
// move of `form.js` is a plain merge. Without `dep.cjs`, the move extracts the namespace at once,
// and this test does not reach the check that runs after module assignment.
import dep from './dep.cjs';

export const value = dep.value;
export const loadForm = () => import('./form.js');
