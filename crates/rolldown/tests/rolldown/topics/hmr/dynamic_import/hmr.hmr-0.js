export async function foo() {
  await import('./exist-dep-cjs').then(console.log)
  await import('./exist-dep-esm').then(console.log)
}

export async function bar() {
  await import('./new-dep-cjs.js').then(console.log)
  await import('./new-dep-esm.js').then(console.log)
}

import.meta.hot.accept((mod) => {
  if (mod) {
    console.log('.hmr', mod.foo)
  }
})

