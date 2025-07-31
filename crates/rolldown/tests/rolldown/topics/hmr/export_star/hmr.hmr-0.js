import { foo } from './sub/index.js'

text('.hmr', foo + ' updated')

function text(el, text) {
  console.log(el, text)
}

import.meta.hot.accept()
