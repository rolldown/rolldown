import 'zx/globals'
import { assertRunningScriptFromRepoRoot } from '../meta/utils.js'
import fsExtra from 'fs-extra'

assertRunningScriptFromRepoRoot()

await $`git clone --branch r108 --depth 1 https://github.com/mrdoob/three.js.git ./temp/three`

// setup three-js

fsExtra.writeFileSync(
  './temp/three/entry.js',
  `import * as three from './src/Three.js'; export { three }`,
)

// setup three-js 10x

fsExtra.ensureDirSync('./temp/three10x')

for (let i = 1; i <= 10; i++) {
  fsExtra.ensureDirSync(`./temp/three10x/copy${i}`)
  fsExtra.copySync('./temp/three/src', `./temp/three10x/copy${i}/`)
}

fsExtra.writeFileSync('./temp/three10x/entry.js', '')
for (let i = 1; i <= 10; i++) {
  fsExtra.appendFileSync(
    './temp/three10x/entry.js',
    `import * as three${i} from './copy${i}/Three.js'; export { three${i} }\n`,
  )
}
