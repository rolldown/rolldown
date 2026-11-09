import { foo } from './cjs.js';
import { shared } from './shared.js';
export const b = () => [foo, shared()];
