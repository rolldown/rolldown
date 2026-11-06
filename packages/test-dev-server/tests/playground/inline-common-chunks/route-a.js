import { shared } from './shared.js';

document.querySelector('.a').textContent = shared('a');

import.meta.hot.accept();
