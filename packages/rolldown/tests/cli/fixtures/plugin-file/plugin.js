module.exports = () => {
  return {
    name: 'injection-a',
    // Resolve `hello` module as `{ hello: 'world' }`.
    resolveId(id) {
      if (id === 'hello') {
        return { id, external: true, moduleSideEffects: false }
      }
    },
    load(id) {
      if (id === 'hello') {
        return "export default { hello: 'world' };"
      }
    },
  }
}
