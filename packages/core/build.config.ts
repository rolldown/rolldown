// build.config.ts
import { defineBuildConfig } from 'unbuild'

export default defineBuildConfig({
  entries: ['./src/index'],
  clean: true,
  declaration: true, // generate .d.ts files
  externals: ['@rolldown/node-binding', 'rollup'],
  rollup: {
    emitCJS: true,
  },
})
