// Records real-rollup chunk counts for each sample in
// rollup/test/chunking-form/samples/. Output: chunking-form-chunk-counts.json.
//
// Scope: chunking-form samples only (the chunk-related test category in
// rollup's own test suite). Sidecar files inside the rollup/ submodule would
// be discarded on the next submodule update, so a single central JSON is used.
//
// Limitations:
// - One format only (`es`); rollup's chunk count generally doesn't depend on it.
// - Plugin/file-resolution failures are recorded with `count: null` + reason
//   so coverage gaps are visible rather than silently skipped.
// - `_test.mjs`-style runtime assertions are not executed; we only count chunks.

import * as fs from 'node:fs'
import { createRequire } from 'node:module'
import * as path from 'node:path'
import { rollup, type InputOptions, type OutputOptions } from 'rollup'
import rollupPkg from 'rollup/package.json' with { type: 'json' }

;(globalThis as Record<string, unknown>).defineTest ??= (c: unknown) => c

const SAMPLES_DIR = path.resolve(
  import.meta.dirname,
  '../../../rollup/test/chunking-form/samples',
)
const OUTPUT_FILE = path.resolve(
  import.meta.dirname,
  './chunking-form-chunk-counts.json',
)

type SampleConfig = {
  description?: string
  skip?: boolean
  solo?: boolean
  options?: InputOptions & { output?: OutputOptions }
}

type Entry =
  | { count: number; description?: string }
  | { count: null; reason: string }

const require_ = createRequire(import.meta.url)

async function recordOne(sampleDir: string): Promise<Entry> {
  const cfgPath = path.join(sampleDir, '_config.js')
  if (!fs.existsSync(cfgPath)) return { count: null, reason: 'no _config.js' }
  let config: SampleConfig
  try {
    delete require_.cache[require_.resolve(cfgPath)]
    config = require_(cfgPath) as SampleConfig
  } catch (e) {
    return {
      count: null,
      reason: `_config.js threw: ${(e as Error).message}`,
    }
  }
  if (config.skip) return { count: null, reason: 'skipped by config' }
  const inputs = config.options?.input ?? [path.join(sampleDir, 'main.js')]
  const prevCwd = process.cwd()
  process.chdir(sampleDir)
  let bundle
  try {
    bundle = await rollup({
      input: inputs as InputOptions['input'],
      onwarn: () => {},
      strictDeprecations: false,
      ...config.options,
    })
    const { output } = await bundle.generate({
      format: 'es',
      exports: 'auto',
      chunkFileNames: 'generated-[name].js',
      ...config.options?.output,
    })
    const count = output.filter((o) => o.type === 'chunk').length
    return { count, description: config.description }
  } catch (e) {
    return { count: null, reason: (e as Error).message }
  } finally {
    await bundle?.close()
    process.chdir(prevCwd)
  }
}

function findSamples(root: string): string[] {
  const out: string[] = []
  function walk(dir: string) {
    if (fs.existsSync(path.join(dir, '_config.js'))) {
      out.push(path.relative(root, dir).split(path.sep).join('/'))
      return
    }
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      if (e.isDirectory() && e.name[0] !== '.' && e.name !== '_expected') {
        walk(path.join(dir, e.name))
      }
    }
  }
  walk(root)
  return out.sort()
}

async function main() {
  const filter = process.argv.slice(2).filter((a) => !a.startsWith('--'))
  if (!fs.existsSync(SAMPLES_DIR)) {
    console.error(
      `[chunking-form-chunks] ${SAMPLES_DIR} missing — submodule not initialized?`,
    )
    process.exit(1)
  }

  const samples = findSamples(SAMPLES_DIR).filter(
    (name) => filter.length === 0 || filter.includes(name),
  )

  const result: Record<string, Entry> = {}
  let ok = 0
  let skipped = 0
  for (const name of samples) {
    const entry = await recordOne(path.join(SAMPLES_DIR, name))
    result[name] = entry
    if (entry.count === null) {
      console.warn(`[skip] ${name}: ${(entry as { reason: string }).reason}`)
      skipped++
    } else {
      ok++
    }
  }

  fs.writeFileSync(
    OUTPUT_FILE,
    JSON.stringify(
      { rollupVersion: rollupPkg.version, samples: result },
      null,
      2,
    ) + '\n',
  )
  console.log(
    `Wrote ${OUTPUT_FILE}: ${ok} recorded, ${skipped} skipped, ${samples.length} total.`,
  )
}

main().catch((e) => {
  console.error(e)
  process.exit(1)
})
