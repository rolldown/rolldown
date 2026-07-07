import { interopValue } from './included-forwarding-barrel.js';
import './e.js';
import { forwarded } from './forwarding-barrel.js';
import { value as first } from './p.js';
import { value as second } from './p.js';

console.log('MAIN', first, second, forwarded, interopValue);
void import('./dynamic.js');
