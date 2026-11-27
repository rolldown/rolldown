import { value } from './shared.js';

globalThis.takesNothingLog.push(`main:${value}`);
export const load = () => import('./dynamic.js');
