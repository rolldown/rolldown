// Control: a plain self-accepting module; its edits hot-update and settle.
export const plain = 'plain-v1';

document.querySelector('.plain').textContent = plain;

import.meta.hot?.accept();
