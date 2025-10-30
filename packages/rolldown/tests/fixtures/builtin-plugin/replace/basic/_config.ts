import { defineTest } from 'rolldown-tests';
import { replacePlugin } from 'rolldown/experimental';

export default defineTest({
  config: {
    plugins: [
      replacePlugin({
        'process.env.NODE_ENV': JSON.stringify('production'),
      }),
    ],
  },
});
