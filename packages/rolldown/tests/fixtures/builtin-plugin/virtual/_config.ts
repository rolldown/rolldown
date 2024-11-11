import { virtualPlugin } from 'rolldown/experimental'
import { defineTest } from '@tests'

export default defineTest({
  config: {
    input: 'src/entry.js',
    plugins: [
      virtualPlugin({
        batman: `export default 'na na na na na'`,
        'src/robin.js': `export default 'batman'`,
      }),
    ],
  },
})
