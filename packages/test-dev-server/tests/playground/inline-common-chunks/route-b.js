import { shared } from './shared.js';

document.querySelector('.b').textContent = shared('b');

import.meta.hot.accept();
