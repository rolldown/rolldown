import type { RollupOptions } from '../../../../src'

const config: RollupOptions = {
  external: [/external/, 'external-a'],
  output: {
    banner: '// banner test',
  }
}

export default {
  config,
}
