export const foo = 'hello

text('.hmr', foo)

function text(el, text) {
  console.log(el, text)
}

import.meta.hot.accept((mod) => {
  if (mod) {
    console.log('.hmr', mod.foo)
  }
})
