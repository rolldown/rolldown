import type { RollupOptions, RollupOutput } from '@rolldown/node'

const config: RollupOptions = {
  external: [/external/, 'external-a'],
}

export default {
  config,
}
