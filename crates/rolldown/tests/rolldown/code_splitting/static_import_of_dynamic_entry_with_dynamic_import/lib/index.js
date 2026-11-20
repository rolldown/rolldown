import { foo } from './foo.js';
import { Bar } from './bar.js';

export const own = { bar: Bar, loadExtra: () => import('./extra.js') };

export { foo, Bar };
