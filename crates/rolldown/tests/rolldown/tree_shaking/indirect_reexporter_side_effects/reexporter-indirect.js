import { foo } from './foo.js';
import { modified } from './helper.js';

foo.indirect = modified();

export { foo };
