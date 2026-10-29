export const foo = 'hello';

export const render = (value) => value;

document.querySelector('.hmr').textContent = render(foo);

import.meta.hot?.accept();
