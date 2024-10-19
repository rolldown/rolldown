import * as diff from 'diff'
import {
  rewriteEsbuild,
  rewriteRolldown,
  defaultRewriteConfig,
} from './rewrite.js'
import { DebugConfig } from './types'
import * as path from 'node:path'
import * as fs from 'node:fs'

type Resolver = (
  esbuildFilename: string,
  rolldownFilename: string,
) => boolean | Record<string, string>
/**
 * our filename generate logic is not the same as esbuild
 * so hardcode some filename remapping
 */
function defaultResolveFunction(
  esbuildFilename: string,
  rolldownFilename: string,
  resolver?: Resolver,
) {
  if (
    typeof resolver === 'function' &&
    resolver(esbuildFilename, rolldownFilename)
  ) {
    return true
  }
  if (resolver && typeof resolver === 'object') {
    if (resolver[esbuildFilename] == rolldownFilename) {
      return true
    }
  }

  if (esbuildFilename === '/out.js' && /entry\.js/.test(rolldownFilename)) {
    return true
  }
  let extractedCaseName = /\/out\/(.*)/.exec(esbuildFilename)?.[1]
  if (extractedCaseName === rolldownFilename) {
    return true
  }
}

export async function diffCase(
  esbuildSnap: {
    name: string
    sourceList: Array<{ name: string; content: string }>
  },
  rolldownSnap: Array<{ filename: string; content: string }> | undefined,
  caseDir: string,
  debugConfig?: DebugConfig,
): Promise<
  | {
      esbuildName: string
      rolldownName: string
      esbuild: string
      rolldown: string
      diff: string
    }[]
  | 'bypass'
  | 'missing'
  | 'same'
> {
  if (!rolldownSnap) {
    return 'missing'
  }
  let diffList = []
  for (let esbuildSource of esbuildSnap.sourceList) {
    let rewriteConfig: any = {}
    let customResolver: Resolver | undefined
    let configPath = path.join(caseDir, 'diff.config.js')
    if (fs.existsSync(configPath)) {
      const mod = (await import(configPath)).default
      rewriteConfig = mod.rewrite ?? {}
      customResolver = mod.resolver
    }
    let matchedSource = rolldownSnap.find((rolldownSource) => {
      if (
        defaultResolveFunction(
          esbuildSource.name,
          rolldownSource.filename,
          customResolver,
        )
      ) {
        return true
      }
      return rolldownSnap.find((snap) => {
        return snap.filename == esbuildSource.name
      })
    }) ?? { content: '', filename: '' }
    let esbuildContent = esbuildSource.content
    let rolldownContent = matchedSource.content
    try {
      esbuildContent = rewriteEsbuild(esbuildSource.content)
      rolldownContent = rewriteRolldown(matchedSource.content, {
        ...defaultRewriteConfig,
        ...rewriteConfig,
      })
    } catch (err) {
      console.error(esbuildSnap.name)
      console.error(esbuildSource.name)
      if (
        debugConfig?.debug &&
        (esbuildSource.name.endsWith('.mjs') ||
          esbuildSource.name.endsWith('.js'))
      ) {
        console.error(`err: `, err)
      }
    }

    if (matchedSource.content !== esbuildSource.content) {
      let structuredPatch = diff.structuredPatch(
        'esbuild',
        'rolldown',
        esbuildContent,
        rolldownContent,
        esbuildSource.name,
        matchedSource.filename,
      )
      let formatDiff = ''
      if (structuredPatch.hunks.length > 0) {
        formatDiff = diff.formatPatch(structuredPatch)
        diffList.push({
          esbuildName: esbuildSource.name,
          rolldownName: matchedSource.filename,
          esbuild: esbuildSource.content,
          rolldown: matchedSource.content,
          diff: formatDiff,
        })
      }
    }
  }
  if (diffList.length === 0) {
    return 'same'
  }
  return diffList
}
