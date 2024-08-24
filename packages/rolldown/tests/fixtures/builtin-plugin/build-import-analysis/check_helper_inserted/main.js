const __vitePreload = (v) => {
  return v()
}
const { foo } = await import('./lib.js')

export { foo }
