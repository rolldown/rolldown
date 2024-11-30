import { rolldown } from 'rolldown'

const build = await rolldown({
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
      banner(chunk) {
        console.log('[banner]')
        console.log(chunk.modules)
      },
    },
  ],
})
const output = await build.write()
console.log(output.output[0].moduleIds)
console.log(output.output[0].modules['\x00virtual:test'].code)
