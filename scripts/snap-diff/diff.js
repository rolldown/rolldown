import * as diff from 'diff'
import { rewriteEsbuild, rewriteRolldown } from './rewrite.js'
/**
 * our filename generate logic is not the same as esbuild
 * so hardcode some filename remapping
 * @param {string} esbuildFilename
 * @param {string} rolldownFilename
 */
function defaultResolveFunction(esbuildFilename, rolldownFilename) {
  if (esbuildFilename === '/out.js' && /entry_js\.*/.test(rolldownFilename)) {
    return true
  }
}
/**
 * TODO: custom resolve
 * @param {{name: string, sourceList: Array<{name: string, content: string}>}} esbuildSnap
 * @param {Array<{filename: string, content: string}> | undefined} rolldownSnap
 * @returns {'missing' | Array<{esbuildName: string, rolldownName: string, esbuild: string, rolldown: string, diff: string}> | 'same'}
 */
export function diffCase(esbuildSnap, rolldownSnap) {
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
    let esbuildContent = rewriteEsbuild(esbuildSource.content)
    let rolldownContent = rewriteRolldown(matchedSource.content)
    if (matchedSource.content !== esbuildSource.content) {
      diffList.push({
        esbuildName: esbuildSource.name,
        rolldownName: matchedSource.filename,
        esbuild: esbuildSource.content,
        rolldown: matchedSource.content,
        diff: diff.createTwoFilesPatch(
          'esbuild',
          'rolldown',
          esbuildContent,
          rolldownContent,
          esbuildSource.name,
          matchedSource.filename,
        ),
      })
    }
  }
  if (diffList.length === 0) {
    return 'same'
  }
  return diffList
}
