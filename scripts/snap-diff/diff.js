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
 * @returns {'missing' | Array<{name: string, diff: any}> | 'same'}
 */
export function diffCase(esbuildSnap, rolldownSnap) {
  if (!rolldownSnap) {
    return 'missing'
  }
  let diff = []
  for (let esbuildSource of esbuildSnap.sourceList) {
    let matchedSource = rolldownSnap.find((rolldownSource) => {
      if (defaultResolveFunction(esbuildSource.name, rolldownSource.filename)) {
        return true
      }
      return rolldownSnap.find((snap) => {
        return snap.filename == esbuildSource.name
      })
    }) ?? { content: '' }
    if (matchedSource.content !== esbuildSource.content) {
      diff.push({
        name: esbuildSource.name,
        rolldownName: matchedSource.filename,
        esbuild: esbuildSource.content,
        rolldown: matchedSource.content,
      })
    }
  }
  if (diff.length === 0) {
    return 'same'
  }
  return diff
}
