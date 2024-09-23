// @ts-check
import * as path from 'node:path'
import * as fs from 'node:fs'
import { parseEsbuildSnap, parseRolldownSnap } from './snap-parser.js'
import { diffCase } from './diff.js'
const esbuildTestDir = path.join(
  import.meta.dirname,
  '../../crates/rolldown/tests/esbuild',
)

/**
 * @param {string[]} includeList
 * @returns {Array<{normalizedName: string, content: string}>}
 */
export function getEsbuildSnapFile(includeList) {
  let dirname = path.resolve(import.meta.dirname, './esbuild-snapshots/')
  let fileList = fs.readdirSync(dirname)
  let ret = fileList
    .filter((filename) => {
      return includeList.length === 0 || includeList.includes(filename)
    })
    .map((filename) => {
      let name = path.parse(filename).name
      let [_, ...rest] = name.split('_')
      let normalizedName = rest.join('_')
      let content = fs.readFileSync(path.join(dirname, filename), 'utf-8')
      return { normalizedName, content }
    })
  return ret
}

/**
 * @param {string[]} includeList
 */
export function run(includeList) {
  let snapfileList = getEsbuildSnapFile(includeList)
  // esbuild snapshot_x.txt
  for (let snapFile of snapfileList) {
    let { normalizedName: snapCategory, content } = snapFile
    let parsedEsbuildSnap = parseEsbuildSnap(content)
    // singleEsbuildSnapshot
    let diffList = []
    for (let snap of parsedEsbuildSnap) {
      let rolldownTestPath = path.join(esbuildTestDir, snapCategory, snap.name)
      let rolldownSnap = getRolldownSnap(rolldownTestPath)
      let parsedRolldownSnap = parseRolldownSnap(rolldownSnap)
      let diffResult = diffCase(snap, parsedRolldownSnap)
      if (typeof diffResult !== 'string') {
        writeDiffToTestcaseDir(rolldownTestPath, diffResult)
      }
      diffList.push({ diffResult, name: snap.name })
    }
    diffList.sort((a, b) => {
      return a.name.localeCompare(b.name)
    })
    let summaryMarkdown = getSummaryMarkdown(diffList)
    fs.writeFileSync(
      path.join(import.meta.dirname, './summary/', `${snapCategory}.md`),
      summaryMarkdown,
    )
  }
}

/**
 * @param {string} caseDir
 *
 */
function getRolldownSnap(caseDir) {
  let artifactsPath = path.join(caseDir, 'artifacts.snap')
  if (fs.existsSync(artifactsPath)) {
    return fs.readFileSync(artifactsPath, 'utf-8')
  }
}

/**
 * @param {string} dir
 * @param {ReturnType<diffCase>} diffResult
 */
function writeDiffToTestcaseDir(dir, diffResult) {
  // this seems redundant, just help ts type infer
  if (typeof diffResult === 'string') {
    return
  }
  let markdown = ''
  for (let d of diffResult) {
    markdown += `## ${d.esbuildName}\n`
    markdown += `### esbuild\n\`\`\`js\n${d.esbuild}\n\`\`\`\n`
    markdown += `### rolldown\n\`\`\`js\n${d.rolldown}\n\`\`\`\n`
    markdown += `### diff\n\`\`\`diff\n${d.diff}\n\`\`\`\n`
  }
  fs.writeFileSync(path.join(dir, 'diff.md'), markdown)
}

/**
 * @param {Array<{diffResult: ReturnType<diffCase>, name: string}>} diffList
 */
function getSummaryMarkdown(diffList) {
  let markdown = `# Failed Cases\n`
  for (let diff of diffList) {
    if (diff.diffResult === 'missing') {
      markdown += `## ${diff.name}\n`
      markdown += `  missing\n`
      continue
    }
    if (diff.diffResult !== 'same') {
      markdown += `## ${diff.name}\n`
      markdown += `  diff\n`
    }
  }
  return markdown
}
