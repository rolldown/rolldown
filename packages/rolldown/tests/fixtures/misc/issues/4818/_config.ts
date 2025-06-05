import { defineTest } from 'rolldown-tests'

export default defineTest({
  config: {
    experimental: {
      hmr: {},
    },
  },
  afterTest: async () => {
    // polyfill for node.js `Websocket`
    (global as any).WebSocket = class {}
    await import('./assert.mjs')
  },
})
