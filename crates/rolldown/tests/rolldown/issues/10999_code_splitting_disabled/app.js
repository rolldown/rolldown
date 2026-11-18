import { h1 } from './h1.js';
import data from './data.json';

export const load = () => import('./h1.js');
export const value = [h1, data];
