import { dirname, normalize, relative, basename } from 'path'
process.cwd = () => '/'
export function getImportPath(
  importerId,
  targetPath,
  stripJsExtension,
  ensureFileName,
) {
  while (targetPath.startsWith('../')) {
    targetPath = targetPath.slice(3)
    importerId = '_/' + importerId
  }
  let relativePath = normalize(relative(dirname(importerId), targetPath))
  if (stripJsExtension && relativePath.endsWith('.js')) {
    relativePath = relativePath.slice(0, -3)
  }
  if (ensureFileName) {
    if (relativePath === '') return '../' + basename(targetPath)
    if (UPPER_DIR_REGEX.test(relativePath)) {
      return [...relativePath.split('/'), '..', basename(targetPath)].join('/')
    }
  }
  return relativePath
    ? relativePath.startsWith('..')
      ? relativePath
      : './' + relativePath
    : '.'
}

console.log(getImportPath('main.js', '..', false, false))
