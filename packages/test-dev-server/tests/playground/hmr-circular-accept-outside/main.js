import { msg } from './mod-a.js';

document.querySelector('.circular').textContent = msg;

import.meta.hot?.accept((mod) => {
  document.querySelector('.circular').textContent = mod.msg;
});
