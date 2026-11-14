import { oops } from './errors.js';
export { forms } from './forms.js';

export function a(name) {
  if (!name) oops({ name: 'a' });
  return name;
}
