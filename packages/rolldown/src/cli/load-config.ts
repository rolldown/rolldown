import path from 'node:path'
import fs from 'node:fs'
import { ConfigExport } from '../types/config-export'
import { pathToFileURL } from 'node:url'
import { rolldown } from '../api/rolldown'
import { OutputChunk } from '../types/rolldown-output'

export async function loadTsConfig(configFile: string): Promise<ConfigExport> {
  const file = await bundleTsConfig(configFile)
  try {
    return (await import(file)).default
  } finally {
    fs.unlink(file, () => {}) // Ignore errors
  }
}

async function bundleTsConfig(configFile: string): Promise<string> {
  const dirnameVarName = 'injected_original_dirname'
  const filenameVarName = 'injected_original_filename'
  const importMetaUrlVarName = 'injected_original_import_meta_url'
  const bundle = await rolldown({
    input: configFile,
    platform: 'node',
    resolve: {
      mainFields: ['main'],
    },
    define: {
      __dirname: dirnameVarName,
      __filename: filenameVarName,
      'import.meta.url': importMetaUrlVarName,
      'import.meta.dirname': dirnameVarName,
      'import.meta.filename': filenameVarName,
    },
    treeshake: false,
    external: [/^[\w@][^:]/], // external bare imports
    plugins: [
      {
        name: 'inject-file-scope-variables',
        transform: {
          filter: { id: /\.[cm]?[jt]s$/ },
          async handler(code, id) {
            const injectValues =
              `const ${dirnameVarName} = ${JSON.stringify(path.dirname(id))};` +
              `const ${filenameVarName} = ${JSON.stringify(id)};` +
              `const ${importMetaUrlVarName} = ${JSON.stringify(
                pathToFileURL(id).href,
              )};`
            return { code: injectValues + code, map: null }
          },
        },
      },
    ],
  })
  const outputDir = path.dirname(configFile)
  const result = await bundle.write({
    dir: outputDir,
    format: 'esm',
    sourcemap: 'inline',
    entryFileNames: 'rolldown.config.[hash].js',
  })
  const fileName = result.output.find(
    (chunk): chunk is OutputChunk => chunk.type === 'chunk' && chunk.isEntry,
  )!.fileName
  return path.join(outputDir, fileName)
}

const SUPPORTED_JS_CONFIG_FORMATS = ['.js', '.mjs', '.cjs']
const SUPPORTED_TS_CONFIG_FORMATS = ['.ts', '.mts', '.cts']
const SUPPORTED_CONFIG_FORMATS = [
  ...SUPPORTED_JS_CONFIG_FORMATS,
  ...SUPPORTED_TS_CONFIG_FORMATS,
]

export async function loadConfig(configPath: string): Promise<ConfigExport> {
  const ext = path.extname(configPath)
  const rawConfigPath = path.resolve(configPath)

  try {
    if (
      SUPPORTED_JS_CONFIG_FORMATS.includes(ext) ||
      (process.env.NODE_OPTIONS?.includes('--import=tsx') &&
        SUPPORTED_TS_CONFIG_FORMATS.includes(ext))
    ) {
      return (await import(rawConfigPath)).default
    } else if (SUPPORTED_TS_CONFIG_FORMATS.includes(ext)) {
      return await loadTsConfig(rawConfigPath)
    } else {
      throw new Error(
        `Unsupported config format. Expected: \`${SUPPORTED_CONFIG_FORMATS.join(',')}\` but got \`${ext}\``,
      )
    }
  } catch (err) {
    throw new Error('Error happened while loading config.', { cause: err })
  }
}
