import { defineConfig } from 'vitest/config'
const IS_CI = process.env.CI
const DISABLE_CODSPEED = process.env.DISABLE_CODSPEED

export default defineConfig(async () => {
  let codspeedPlugin
  if (!DISABLE_CODSPEED && IS_CI) {
    // @ts-expect-error: `@codspeed/vitest-plugin` doesn't specify `types` in `package.json#exports`.
    codspeedPlugin = (await import('@codspeed/vitest-plugin')).default
  }
  return {
    plugins: [codspeedPlugin].filter(Boolean),
  }
})
