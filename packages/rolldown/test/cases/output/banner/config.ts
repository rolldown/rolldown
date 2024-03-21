import type { RollupOptions } from '../../../../src'

const config: RollupOptions = {
  external: [/external/, 'external-a'],
  output: {
    banner: 'banner test\n',
    dir: './dist'
  }
}

export default {
  config,
}
