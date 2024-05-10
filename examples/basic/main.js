// import { rolldown } from 'rolldown'
const { rolldown} = require( 'rolldown')

async function run() {
  let a = await rolldown({
    input: './index.js',
    plugins: [
      {
        name: 'test-plugin',
        generateBundle: (options, bundle, isWrite) => {
          const chunk = bundle['index.js']
          // Mutate chunk
          chunk.code = 'console.error()'
        },
      },
    ],
  })
  const v = await a.write()
  console.log(v.output[0].code)
  v.output[0].code = "1"
  console.log(v.output[0].code)

}

run()