import 'zx/globals'
import { assertRunningScriptFromRepoRoot } from '../meta/utils.js'
import fsExtra from 'fs-extra'
import glob from 'fast-glob'

assertRunningScriptFromRepoRoot()

async function cloneThreeJsIfNotExists() {
  if (!fsExtra.existsSync('./tmp/github/three')) {
    fsExtra.ensureDirSync('./tmp/github')
    await $`git clone --branch r108 --depth 1 https://github.com/mrdoob/three.js.git ./tmp/github/three`
  } else {
    console.log('[skip] three.js already cloned')
  }
}

async function fetchRomeIfNotExists() {
  if (!fsExtra.existsSync('./tmp/github/rome')) {
    fsExtra.ensureDirSync('./tmp/github/rome')
    cd('./tmp/github/rome')
    await $`git init`
    await $`git remote add origin https://github.com/romejs/rome.git`
    await $`git fetch --depth 1 origin d95a3a7aab90773c9b36d9c82a08c8c4c6b68aa5`
    await $`git checkout FETCH_HEAD`
    cd('../../..')
  } else {
    console.log('[skip] rome already cloned')
  }
}

await cloneThreeJsIfNotExists()
await fetchRomeIfNotExists()

// setup three-js

if (fsExtra.existsSync('./tmp/bench/three')) {
  console.log('[skip] setup three already')
} else {
  console.log('Setup `three` in tmp/bench')
  fsExtra.copySync('./tmp/github/three', `./tmp/bench/three`)

  fsExtra.writeFileSync(
    './tmp/bench/three/entry.js',
    `import * as three from './src/Three.js'; export { three }`,
  )
}

// setup three-js 10x

if (fsExtra.existsSync('./tmp/bench/three10x')) {
  console.log('[skip] setup three10x already')
} else {
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
}

// setup rome

if (fsExtra.existsSync('./tmp/bench/rome')) {
  console.log('[skip] setup rome already')
} else {
  console.log('Setup `rome` in tmp/bench')

  fsExtra.copySync('./tmp/github/rome/packages', './tmp/bench/rome/src/', {
    filter(src) {
      // an error happens on windows without this filter
      return !src.includes('.bin')
    },
  })
  fsExtra.writeFileSync(
    './tmp/bench/rome/src/entry.ts',
    'import "rome/bin/rome"',
  )
  fsExtra.writeFileSync(
    './tmp/bench/rome/src/tsconfig.json',
    JSON.stringify(
      {
        compilerOptions: {
          sourceMap: true,
          esModuleInterop: true,
          resolveJsonModule: true,
          moduleResolution: 'node',
          target: 'es2019',
          module: 'commonjs',
          baseUrl: '.',
        },
      },
      null,
      2,
    ),
  )

  // replace `export default function name()` with `export default function()`
  // rome uses a same identifier as a type and a value and that chokes babel
  const files = await glob('./tmp/bench/rome/src/**/*.ts')
  const problematicExportDefaultRE = /export default function \w+\(/
  for (const file of files) {
    const content = await fsExtra.readFile(file, 'utf8')
    if (problematicExportDefaultRE.test(content)) {
      await fsExtra.writeFile(
        file,
        content.replace(problematicExportDefaultRE, 'export default function('),
      )
    }
  }
  // also replace some additional things in `@romejs/js-formatter/node/parentheses.ts`
  {
    const file = './tmp/bench/rome/src/@romejs/js-formatter/node/parentheses.ts'
    const content = await fsExtra.readFile(file, 'utf8')
    const newContent = content.replace(
      /import \{((?:.|\n)*)\} from '@romejs\/js-ast';/,
      "import type {$1} from '@romejs/js-ast';",
    )
    await fsExtra.writeFile(file, newContent)
  }
}
