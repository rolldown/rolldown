import.meta.hot.accept((mod) => {
  if (mod) {
    console.log('.hmr', mod.foo)
  }
})
export async function foo() {
  const esm = await import('./new-dep-esm.js')
  const cjs = await import('./new-dep-cjs.js')
  console.log(esm, cjs)
}