const plugin = () => ({
  name: 'runtime-plugin',
  loadEntry() {
    return globalThis.remote
  },
})

export default plugin
