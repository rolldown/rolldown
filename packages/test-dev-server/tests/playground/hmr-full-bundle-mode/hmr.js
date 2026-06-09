export const foo = 'hello';

export const render = (value) => value;

text('.hmr', render(foo));

function text(el, text) {
  document.querySelector(el).textContent = text;
}

import.meta.hot?.accept((mod) => {
  if (mod) {
    text('.hmr', mod.render(mod.foo));
  }
});
