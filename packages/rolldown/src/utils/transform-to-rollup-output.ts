import type {
  RolldownOutput,
  RolldownOutputAsset,
  RolldownOutputChunk,
} from '../types/rolldown-output'
import type { OutputBundle } from '../types/output-bundle'
import type {
  BindingOutputAsset,
  BindingOutputChunk,
  BindingOutputs,
  JsChangedOutputs,
  JsOutputAsset,
  JsOutputChunk,
} from '../binding'
import {
  AssetSource,
  bindingAssetSource,
  transformAssetSource,
} from './asset-source'
import { bindingifySourcemap } from '../types/sourcemap'
import { transformToRenderedModule } from './transform-rendered-module'
import { RollupLog } from '../rollup'

function transformToRollupOutputChunk(
  bindingChunk: BindingOutputChunk,
  changed?: ChangedOutputs,
): RolldownOutputChunk {
  const chunk = {
    type: 'chunk',
    get code() {
      return bindingChunk.code
    },
    fileName: bindingChunk.fileName,
    name: bindingChunk.name,
    get modules() {
      return Object.fromEntries(
        Object.entries(bindingChunk.modules).map(([key, value]) => [
          key,
          transformToRenderedModule(value),
        ]),
      )
    },
    get imports() {
      return bindingChunk.imports
    },
    get dynamicImports() {
      return bindingChunk.dynamicImports
    },
    exports: bindingChunk.exports,
    isEntry: bindingChunk.isEntry,
    facadeModuleId: bindingChunk.facadeModuleId || null,
    isDynamicEntry: bindingChunk.isDynamicEntry,
    get moduleIds() {
      return bindingChunk.moduleIds
    },
    get map() {
      return bindingChunk.map ? JSON.parse(bindingChunk.map) : null
    },
    sourcemapFileName: bindingChunk.sourcemapFileName || null,
    preliminaryFileName: bindingChunk.preliminaryFileName,
  } as RolldownOutputChunk
  const cache: Record<string | symbol, any> = {}
  return new Proxy(chunk, {
    get(target, p) {
      if (p in cache) {
        return cache[p]
      }
      return target[p as keyof RolldownOutputChunk]
    },
    set(target, p, newValue): boolean {
      cache[p] = newValue
      changed?.updated.add(bindingChunk.fileName)
      return true
    },
  })
}

function transformToRollupOutputAsset(
  bindingAsset: BindingOutputAsset,
  changed?: ChangedOutputs,
): RolldownOutputAsset {
  const asset = {
    type: 'asset',
    fileName: bindingAsset.fileName,
    originalFileName: bindingAsset.originalFileName || null,
    get source(): AssetSource {
      return transformAssetSource(bindingAsset.source)
    },
    name: bindingAsset.name ?? undefined,
  } as RolldownOutputAsset
  const cache: Record<string | symbol, any> = {}
  return new Proxy(asset, {
    get(target, p) {
      if (p in cache) {
        return cache[p]
      }
      return target[p as keyof RolldownOutputAsset]
    },
    set(target, p, newValue): boolean {
      cache[p] = newValue
      changed?.updated.add(bindingAsset.fileName)
      return true
    },
  })
}

export function transformToRollupOutput(
  output: BindingOutputs,
  changed?: ChangedOutputs,
): RolldownOutput {
  handleOutputErrors(output)
  const { chunks, assets } = output
  return {
    output: [
      ...chunks.map((chunk) => transformToRollupOutputChunk(chunk, changed)),
      ...assets.map((asset) => transformToRollupOutputAsset(asset, changed)),
    ],
  } as RolldownOutput
}

export function handleOutputErrors(output: BindingOutputs) {
  const rawErrors = output.errors
  if (rawErrors.length > 0) {
    const errors = rawErrors.map((e) =>
      e instanceof Error
        ? e
        : // strip stacktrace of errors from native diagnostics
          Object.assign(new Error(), e, { stack: undefined }),
    )
    // based on https://github.com/evanw/esbuild/blob/9eca46464ed5615cb36a3beb3f7a7b9a8ffbe7cf/lib/shared/common.ts#L1673
    // combine error messages as a top level error
    let summary = `Build failed with ${errors.length} error${errors.length < 2 ? '' : 's'}:\n`
    for (let i = 0; i < errors.length; i++) {
      if (i >= 5) {
        summary += '\n...'
        break
      }
      summary += getErrorMessage(errors[i]) + '\n'
    }
    const wrapper = new Error(summary)
    // expose individual errors as getters so that
    // `console.error(wrapper)` doesn't expand unnecessary details
    // when they are already presented in `wrapper.message`
    Object.defineProperty(wrapper, 'errors', {
      configurable: true,
      enumerable: true,
      get: () => errors,
      set: (value) =>
        Object.defineProperty(wrapper, 'errors', {
          configurable: true,
          enumerable: true,
          value,
        }),
    })
    throw wrapper
  }
}

function getErrorMessage(e: RollupLog) {
  let prefix = ''
  if (e.plugin) {
    prefix += `[plugin ${e.plugin}]`
  }
  const id = e.id ?? e.loc?.file
  if (id) {
    prefix += ' ' + id
    if (e.loc) {
      prefix += `:${e.loc.line}:${e.loc.column}`
    }
  }
  if (prefix) {
    prefix += '\n'
  }
  return prefix + (e.stack ?? e.message)
}

export function transformToOutputBundle(
  output: BindingOutputs,
  changed: ChangedOutputs,
): OutputBundle {
  const bundle = Object.fromEntries(
    transformToRollupOutput(output, changed).output.map((item) => [
      item.fileName,
      item,
    ]),
  )
  return new Proxy(bundle, {
    deleteProperty(target, property): boolean {
      if (typeof property === 'string') {
        changed.deleted.add(property)
      }
      return true
    },
  })
}

export interface ChangedOutputs {
  updated: Set<string>
  deleted: Set<string>
}

// TODO find a way only transfer the changed part to rust side.
export function collectChangedBundle(
  changed: ChangedOutputs,
  bundle: OutputBundle,
): JsChangedOutputs {
  const assets: Array<JsOutputAsset> = []
  const chunks: Array<JsOutputChunk> = []

  for (const key in bundle) {
    if (changed.deleted.has(key) || !changed.updated.has(key)) {
      continue
    }
    const item = bundle[key]
    if (item.type === 'asset') {
      assets.push({
        filename: item.fileName,
        originalFileName: item.originalFileName || undefined,
        source: bindingAssetSource(item.source),
        name: item.name,
      })
    } else {
      chunks.push({
        code: item.code,
        filename: item.fileName,
        name: item.name,
        isEntry: item.isEntry,
        exports: item.exports,
        modules: Object.fromEntries(
          Object.entries(item.modules).map(([key, _]) => [key, {} as any]),
        ),
        imports: item.imports,
        dynamicImports: item.dynamicImports,
        facadeModuleId: item.facadeModuleId || undefined,
        isDynamicEntry: item.isDynamicEntry,
        moduleIds: item.moduleIds,
        map: bindingifySourcemap(item.map),
        sourcemapFilename: item.sourcemapFileName || undefined,
        preliminaryFilename: item.preliminaryFileName,
      })
    }
  }
  return {
    assets,
    chunks,
    deleted: Array.from(changed.deleted),
  }
}
