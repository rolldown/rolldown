import.meta.hot.accept((mod) => {
  if (mod) {
    console.log('.hmr', mod.foo)
  }
})
export async function foo() {
  const mod = await import('./new-dep.js')
  console.log(mod.value)
}