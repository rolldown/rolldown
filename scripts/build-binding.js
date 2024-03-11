import 'zx/globals'
import nodePath from 'path'
import { repoRoot } from './meta/constants.js'

$.cwd = repoRoot
// process.chdir(nodePath.join(repoRoot, 'packages/node').normalize())
process.chdir(repoRoot)

const manifestPath = nodePath.join('.', 'crates/rolldown_binding/Cargo.toml').normalize()

const distPath = nodePath.join('.', 'packages/node/src').normalize()

await $`yarn napi build --manifest-path ${manifestPath} --platform -p rolldown_binding --js ${path.join(distPath, 'binding').normalize()} --dts ${path.join(distPath, 'binding.d.ts').normalize()} -o .`
