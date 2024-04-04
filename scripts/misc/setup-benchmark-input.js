import 'zx/globals'
import { assertRunningScriptFromRepoRoot } from '../meta/utils.js'
import fsExtra from 'fs-extra'

assertRunningScriptFromRepoRoot()

async function cloneThreeJsIfNotExists() {
  if (!fsExtra.existsSync('./tmp/github/three')) {
    fsExtra.ensureDirSync('./tmp/github')
    await $`git clone --branch r108 --depth 1 https://github.com/mrdoob/three.js.git ./tmp/github/three`
  } else {
    console.log('[skip] three.js already cloned')
  }
}

await cloneThreeJsIfNotExists()

// setup three-js

console.log('Setup `three` in tmp/bench')

fsExtra.copySync('./tmp/github/three', `./tmp/bench/three`)

fsExtra.writeFileSync(
  './tmp/bench/three/entry.js',
  `import * as three from './src/Three.js'; export { three }`,
)

// setup three-js 10x

console.log('Setup `three10x` in tmp/bench')

fsExtra.ensureDirSync('./tmp/bench/three10x')

for (let i = 1; i <= 10; i++) {
  fsExtra.ensureDirSync(`./tmp/bench/three10x/copy${i}`)
  fsExtra.copySync('./tmp/bench/three/src', `./tmp/bench/three10x/copy${i}/`)
}

fsExtra.writeFileSync('./tmp/bench/three10x/entry.js', '')
for (let i = 1; i <= 10; i++) {
  fsExtra.appendFileSync(
    './tmp/bench/three10x/entry.js',
    `import * as three${i} from './copy${i}/Three.js'; export { three${i} }\n`,
  )
}
