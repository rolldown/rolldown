import './effect.js';
import './setup.js';
import { value } from './barrel.js';

export const seen = globalThis.order.join(',');
export const loadSetup = () => import('./setup.js');
export const loadBarrel = () => import('./barrel.js');
export { value };
