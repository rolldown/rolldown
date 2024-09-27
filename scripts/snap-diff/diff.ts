import * as diff from 'diff'
import { rewriteEsbuild, rewriteRolldown } from './rewrite.js'
/**
 * our filename generate logic is not the same as esbuild
 * so hardcode some filename remapping
 */
function defaultResolveFunction(
  esbuildFilename: string,
  rolldownFilename: string,
) {
  if (esbuildFilename === '/out.js' && /entry_js\.*/.test(rolldownFilename)) {
    return true
  }
}
/**
 * TODO: custom resolve
 */
export function diffCase(
  esbuildSnap: {
    name: string
    sourceList: Array<{ name: string; content: string }>
  },
  rolldownSnap: Array<{ filename: string; content: string }> | undefined,
):
  | 'bypass'
  | 'missing'
  | Array<{
      esbuildName: string
      rolldownName: string
      esbuild: string
      rolldown: string
      diff: string
    }>
  | 'same' {
  if (!rolldownSnap) {
    return 'missing'
  }
  let diffList = []
  for (let esbuildSource of esbuildSnap.sourceList) {
    let matchedSource = rolldownSnap.find((rolldownSource) => {
      if (defaultResolveFunction(esbuildSource.name, rolldownSource.filename)) {
        return true
      }
      return rolldownSnap.find((snap) => {
        return snap.filename == esbuildSource.name
      })
    }) ?? { content: '', filename: '' }
    let esbuildContent = rewriteEsbuild(esbuildSource.content.trim())
    let rolldownContent = rewriteRolldown(matchedSource.content.trim())

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
