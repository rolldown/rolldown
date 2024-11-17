globalThis.input = {}
await import('./dist/entry.js')
if (!await input.works) throw new Error('Test did not pass')