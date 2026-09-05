/* oxlint-disable */
import { msg } from './content.js';
import { upper } from './content.js?upper';

document.querySelector('.base').textContent = msg;
document.querySelector('.variant').textContent = upper;

import.meta.hot?.accept();
