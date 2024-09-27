import * as path from 'node:path'
import * as fs from 'node:fs'
import { parseEsbuildSnap, parseRolldownSnap } from './snap-parser.js'
import { diffCase } from './diff'
const esbuildTestDir = path.join(
  import.meta.dirname,
  '../../crates/rolldown/tests/esbuild',
)

export function getEsbuildSnapFile(
  includeList: string[],
): Array<{ normalizedName: string; content: string }> {
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

export function run(includeList: string[]) {
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
      // if the testDir has a `bypass.md`, we skip generate `diff.md`,
      // append the diff result to `bypass.md` instead
      let bypassMarkdownPath = path.join(rolldownTestPath, 'bypass.md')
      let diffMarkdownPath = path.join(rolldownTestPath, 'diff.md')

      if (fs.existsSync(bypassMarkdownPath)) {
        if (fs.existsSync(diffMarkdownPath)) {
          fs.rmSync(diffMarkdownPath, {})
        }
        updateBypassMarkdown(bypassMarkdownPath, diffResult)
        diffResult = 'bypass'
      } else {
        if (typeof diffResult !== 'string') {
          writeDiffMarkdownToTestcaseDir(diffResult, rolldownTestPath)
        } else {
          if (diffResult === 'same' && fs.existsSync(diffMarkdownPath)) {
            // this happens when we fixing some issues and the snapshot is align with esbuild,
            fs.rmSync(diffMarkdownPath, {})
          }
        }
      }
      diffList.push({ diffResult, name: snap.name })
    }
    diffList.sort((a, b) => {
      return a.name.localeCompare(b.name)
    })
    let summaryMarkdown = getSummaryMarkdown(diffList, snapCategory)
    fs.writeFileSync(
      path.join(import.meta.dirname, './summary/', `${snapCategory}.md`),
      summaryMarkdown,
    )
  }
}

function getRolldownSnap(caseDir: string) {
  let artifactsPath = path.join(caseDir, 'artifacts.snap')
  if (fs.existsSync(artifactsPath)) {
    return fs.readFileSync(artifactsPath, 'utf-8')
  }
}

function writeDiffMarkdownToTestcaseDir(
  diffResult: ReturnType<typeof diffCase>,
  dir: string,
) {
  // this seems redundant, just help ts type infer
  if (typeof diffResult === 'string') {
    return
  }
  let markdown = getDiffMarkdown(diffResult)
  fs.writeFileSync(path.join(dir, 'diff.md'), markdown)
}

function getDiffMarkdown(diffResult: ReturnType<typeof diffCase>) {
  if (typeof diffResult === 'string') {
    throw new Error('diffResult should not be string')
  }
  let markdown = ''
  for (let d of diffResult) {
    markdown += `## ${d.esbuildName}\n`
    markdown += `### esbuild\n\`\`\`js\n${d.esbuild}\n\`\`\`\n`
    markdown += `### rolldown\n\`\`\`js\n${d.rolldown}\n\`\`\`\n`
    markdown += `### diff\n\`\`\`diff\n${d.diff}\n\`\`\`\n`
  }
  return markdown
}

function getSummaryMarkdown(
  diffList: Array<{ diffResult: ReturnType<typeof diffCase>; name: string }>,
  snapshotCategory: string,
) {
  let bypassList = []
  let failedList = []
  let passList = []
  for (let diff of diffList) {
    if (diff.diffResult === 'bypass') {
      bypassList.push(diff)
    } else if (diff.diffResult === 'same') {
      passList.push(diff)
    } else {
      failedList.push(diff)
    }
  }
  let markdown = `# Failed Cases\n`
  for (let diff of failedList) {
    let testDir = path.join(esbuildTestDir, snapshotCategory, diff.name)
    let relativePath = path.relative(
      path.join(import.meta.dirname, 'summary'),
      testDir,
    )
    const posixPath = relativePath.replaceAll('\\', '/')
    if (diff.diffResult === 'missing') {
      markdown += `## ${diff.name}\n`
      markdown += `  missing\n`
      continue
    }
    if (diff.diffResult !== 'same') {
      markdown += `## [${diff.name}](${posixPath}/diff.md)\n`
      markdown += `  diff\n`
    }
  }

  markdown += `# Passed Cases\n`
  for (let diff of passList) {
    let testDir = path.join(esbuildTestDir, snapshotCategory, diff.name)
    let relativePath = path.relative(
      path.join(import.meta.dirname, 'summary'),
      testDir,
    )
    const posixPath = relativePath.replaceAll('\\', '/')
    markdown += `## [${diff.name}](${posixPath})\n`
  }

  markdown += `# Bypassed Cases\n`
  for (let diff of bypassList) {
    let testDir = path.join(esbuildTestDir, snapshotCategory, diff.name)
    let relativePath = path.relative(
      path.join(import.meta.dirname, 'summary'),
      testDir,
    )
    const posixPath = relativePath.replaceAll('\\', '/')
    markdown += `## [${diff.name}](${posixPath}/bypass.md)\n`
  }

  return markdown
}

function updateBypassMarkdown(
  bypassMarkdownPath: string,
  diffResult: ReturnType<typeof diffCase>,
) {
  let bypassContent = fs.readFileSync(bypassMarkdownPath, 'utf-8')

  let res = /# Diff/.exec(bypassContent)
  if (res) {
    bypassContent = bypassContent.slice(0, res.index)
  }
  let diffMarkdown = getDiffMarkdown(diffResult)
  bypassContent = bypassContent.trimEnd()
  bypassContent += '\n# Diff\n'
  bypassContent += diffMarkdown
  fs.writeFileSync(bypassMarkdownPath, bypassContent)
}
