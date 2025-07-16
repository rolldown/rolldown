import './exist-dep-cjs'
import './exist-dep-esm'
import './new-dep-cjs'
import './new-dep-esm'

import.meta.hot.accept((mod) => {
  if (mod) {
    console.log('.hmr', mod.foo)
  }
})
