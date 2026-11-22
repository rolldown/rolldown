import defer * as b from './barrel.js';
import { readBarrel } from './consumer.js';

export const readDeferred = () => [b.barrelValue, b.starValue];
export { readBarrel };
