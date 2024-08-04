import('./shared').then((imported) => {
  assert.strictEqual(imported.shared, 'shared')
})

export const main = 'main'
