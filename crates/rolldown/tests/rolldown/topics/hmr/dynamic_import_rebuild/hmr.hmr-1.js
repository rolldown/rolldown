export async function foo() {
  await import('./exist-dep-cjs').then(console.log)
  await import('./exist-dep-esm').then(console.log)
}
