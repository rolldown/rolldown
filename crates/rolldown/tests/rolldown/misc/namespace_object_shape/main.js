import * as namespace from './target.js';
import * as names from './names.js';

export { namespace, names };
export const load = () => import('./target.js');
