import { foo } from './sub/index.js'

text('.hmr', foo)

function text(el, text) {
  console.log(el, text)
}

import.meta.hot.accept()
