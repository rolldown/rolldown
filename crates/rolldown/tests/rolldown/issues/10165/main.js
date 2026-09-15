import { F, f, used } from './module.js';

if (!used) {
  throw new Error('destructuring effect was dropped');
}

export { F, f, used };
