import { defineConfig } from 'rolldown'

export default defineConfig({
  input: './main.js',
  plugins: [
    {
      name: 'repro',
      resolveId(source) {
        if (source === 'virtual:test') {
          return '\0' + source
        }
      },
      load(id) {
        if (id === '\0virtual:test') {
          return `export default "hello"`
        }
      },
      buildEnd() {
        console.log('[buildEnd]')
      },
      renderStart() {
        console.log('[renderStart]')
      },
      renderChunk(_, chunk) {
        console.log('[renderChunk]')
        console.log(chunk.moduleIds)
        console.log(chunk.modules)
      },
    },
  ],
})
