export { then as keepThen } from './target.js';
import('./unused.js');
export const load = () => import('./target.js').then((ns) => ns.value);
