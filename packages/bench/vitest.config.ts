import { defineConfig } from 'vitest/config'

const IS_CI = process.env.CI

export default defineConfig(async () => {
  let codspeedPlugin
  if (IS_CI) {
    // @ts-expect-error: `@codspeed/vitest-plugin` doesn't specify `types` in `package.json#exports`.
    codspeedPlugin = (await import('@codspeed/vitest-plugin')).default
    console.log('Codspeed plugin enabled')
  }
  return {
    plugins: [codspeedPlugin].filter(Boolean),
  }
})
