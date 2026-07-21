import { value } from './common-child.js';

window.__executed.push('parent-executed');
document.querySelector('.child').textContent = value;

import.meta.hot?.accept();
