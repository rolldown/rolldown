import { replacePlugin } from 'rolldown/experimental'
import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    plugins: [
      replacePlugin({
        'process.env.NODE_ENV': JSON.stringify('production'),
      }),
    ],
  },
})
