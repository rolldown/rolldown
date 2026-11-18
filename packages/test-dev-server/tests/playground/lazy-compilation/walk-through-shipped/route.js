import { shared } from './shared.js';
import { dep } from './dep.js';

export const route = `${shared}+${dep}`;

import.meta.hot?.accept();
