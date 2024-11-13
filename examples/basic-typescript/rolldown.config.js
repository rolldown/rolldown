import { defineConfig } from 'rolldown'

export default defineConfig({
  input: {
    entry: './index.ts',
  },
  resolve: {
    // This needs to be explicitly set for now because oxc resolver doesn't
    // assume default exports conditions. Rolldown will ship with a default that
    // aligns with Vite in the future.
    conditionNames: ['import'],
  },
  plugins: [
    {
      name: 'repro',
      async resolveId() {
        await testFn()
      },
    },
  ],
})

async function testFn() {
  await Promise.resolve()
  await testMoreFn()
}

async function testMoreFn() {
  await Promise.resolve()
  throw new Error('test error!')
}
