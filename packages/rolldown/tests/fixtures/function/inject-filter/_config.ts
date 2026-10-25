import { defineTest } from 'rolldown-tests';

export default defineTest({
  config: {
    transform: {
      inject: {
        // import { useKey } from './composable'
        useKey: ['./composable', 'useKey'],
      },
      injectFilter: { exclude: /[\\/]vendor[\\/]/ },
    },
    external: ['node:assert'],
  },
});
