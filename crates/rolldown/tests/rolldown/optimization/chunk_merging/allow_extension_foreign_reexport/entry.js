export { task } from './task.js';
export const load = () => [import('./seg_task.js'), import('./seg_other.js')];
