import { foo } from './cjs.js';
import { shared } from './shared.js';
export const a = () => [foo, shared()];
