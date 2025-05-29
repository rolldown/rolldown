import * as fs from 'node:fs';
import * as path from 'node:path';
import { aggregateReason } from './aggregate-reason.js';
import { diffCase } from './diff';
import { parseEsbuildSnap, parseRolldownSnap } from './snap-parser.js';
import { DebugConfig, UnwrapPromise } from './types';
const esbuildTestDir = path.join(
  import.meta.dirname,
  '../../crates/rolldown/tests/esbuild',
);

function getEsbuildSnapFile(
  includeList: string[],
): Array<{ normalizedName: string; content: string }> {
  let dirname = path.resolve(import.meta.dirname, './esbuild-snapshots/');
  let fileList = fs.readdirSync(dirname);
  let ret = fileList
    .filter((filename) => {
      return includeList.length === 0 || includeList.includes(filename);
    })
    .map((filename) => {
      let name = path.parse(filename).name;
      let [_, ...rest] = name.split('_');
      let normalizedName = rest.join('_');
      let content = fs.readFileSync(path.join(dirname, filename), 'utf-8');
      return { normalizedName, content };
    });
  return ret;
}
type AggregateStats = {
  stats: Stats;
  details: Record<string, Stats>;
};

type Stats = {
  pass: number;
  bypass: number;
  failed: number;
  total: number;
};
export async function run(includeList: string[], debugConfig: DebugConfig) {
  let aggregatedStats: AggregateStats = {
    stats: {
      pass: 0,
      bypass: 0,
      failed: 0,
      total: 0,
    },
    details: {},
  };
  let snapfileList = getEsbuildSnapFile(includeList);
  // esbuild snapshot_x.txt
  for (let snapFile of snapfileList) {
    if (debugConfig?.debug) {
      console.log('category:', snapFile.normalizedName);
    }
    let { normalizedName: snapCategory, content } = snapFile;
    let parsedEsbuildSnap = parseEsbuildSnap(content);
    // singleEsbuildSnapshot
    let diffList = [];
    for (let snap of parsedEsbuildSnap) {
      if (
        debugConfig.caseNames?.length > 0 &&
        !debugConfig.caseNames.includes(snap.name)
      ) {
        continue;
      }
      if (debugConfig.debug) {
        console.log('processing', snap.name);
      }
      let rolldownTestPath = path.join(esbuildTestDir, snapCategory, snap.name);
      let rolldownSnap = getRolldownSnap(rolldownTestPath);
      let parsedRolldownSnap = parseRolldownSnap(rolldownSnap);
      let diffResult = await diffCase(
        snap,
        parsedRolldownSnap,
        rolldownTestPath,
        debugConfig,
      );
      // if the testDir has a `bypass.md`, we skip generate `diff.md`,
      // append the diff result to `bypass.md` instead
      let bypassMarkdownPath = path.join(rolldownTestPath, 'bypass.md');
      let diffMarkdownPath = path.join(rolldownTestPath, 'diff.md');
      if (fs.existsSync(bypassMarkdownPath) && typeof diffResult === 'object') {
        if (fs.existsSync(diffMarkdownPath)) {
          fs.rmSync(diffMarkdownPath, {});
        }
        updateBypassOrDiffMarkdown(bypassMarkdownPath, diffResult);
        diffResult = 'bypass';
      } else if (typeof diffResult === 'string') {
        if (fs.existsSync(bypassMarkdownPath)) {
          fs.rmSync(bypassMarkdownPath, {});
        }
        if (fs.existsSync(diffMarkdownPath)) {
          fs.rmSync(diffMarkdownPath, {});
        }
      } else {
        updateBypassOrDiffMarkdown(
          path.join(rolldownTestPath, 'diff.md'),
          diffResult,
        );
      }
      diffList.push({ diffResult, name: snap.name });
    }
    diffList.sort((a, b) => {
      return a.name.localeCompare(b.name);
    });
    let summary = getSummaryMarkdownAndStats(diffList, snapCategory);
    fs.writeFileSync(
      path.join(import.meta.dirname, './summary/', `${snapCategory}.md`),
      summary.markdown,
    );
    aggregatedStats.details[snapCategory] = summary.stats;
    aggregatedStats.stats.total += summary.stats.total;
    aggregatedStats.stats.pass += summary.stats.pass;
    aggregatedStats.stats.bypass += summary.stats.bypass;
    aggregatedStats.stats.failed += summary.stats.failed;
  }
  let unsupportedCaseCount = generateAggregateMarkdown();
  generateStatsMarkdown(aggregatedStats, unsupportedCaseCount);
}

function generateAggregateMarkdown() {
  let entries = aggregateReason();
  let markdown = '# Aggregate Reason\n';
  let unsupportedCase = 0;
  let markdownSkipUnsupported = '# Aggregate Reason\n';
  for (let [reason, caseDirs] of entries) {
    markdown += `## ${reason}\n`;
    for (let dir of caseDirs) {
      markdown += `- ${dir}\n`;
    }
    if (reason.startsWith('not support')) {
      unsupportedCase += caseDirs.length;
      continue;
    }
    markdownSkipUnsupported += `## ${reason}\n`;
    for (let dir of caseDirs) {
      markdownSkipUnsupported += `- ${dir}\n`;
    }
  }

  fs.writeFileSync(
    path.resolve(import.meta.dirname, './stats/aggregated-reason.md'),
    markdown,
  );
  fs.writeFileSync(
    path.resolve(
      import.meta.dirname,
      './stats/aggregated-reason-without-not-support.md',
    ),
    markdownSkipUnsupported,
  );
  return unsupportedCase;
}

function getRolldownSnap(caseDir: string) {
  let artifactsPath = path.join(caseDir, 'artifacts.snap');
  if (fs.existsSync(artifactsPath)) {
    return fs.readFileSync(artifactsPath, 'utf-8');
  }
}

function getDiffMarkdown(
  diffResult: UnwrapPromise<ReturnType<typeof diffCase>>,
) {
  if (typeof diffResult === 'string') {
    throw new Error('diffResult should not be string');
  }
  let markdown = '';
  for (let d of diffResult) {
    markdown += `## ${d.esbuildName}\n`;
    markdown += `### esbuild\n\`\`\`js\n${d.esbuild}\n\`\`\`\n`;
    markdown += `### rolldown\n\`\`\`js\n${d.rolldown}\n\`\`\`\n`;
    markdown += `### diff\n\`\`\`diff\n${d.diff}\n\`\`\`\n`;
  }
  return markdown;
}

function generateStatsMarkdown(
  aggregateStats: AggregateStats,
  unsupportedCaseCount: number,
) {
  const { stats, details } = aggregateStats;
  let markdown = '';

  markdown += `# Compatibility metric\n`;
  markdown += `- total: ${stats.total}\n`;
  markdown += `- passed: ${stats.total - stats.failed}\n`;
  markdown += `- passed ratio: ${
    (((stats.total - stats.failed) / stats.total) * 100).toFixed(2)
  }%\n`;

  let totalWithoutNotSupport = stats.total - unsupportedCaseCount;
  markdown += `# Compatibility metric without not supported case\n`;
  markdown += `- total: ${totalWithoutNotSupport}\n`;
  markdown += `- passed: ${stats.total - stats.failed}\n`;
  markdown += `- passed ratio: ${
    (((stats.total - stats.failed) / totalWithoutNotSupport) * 100).toFixed(2)
  }%\n`;

  markdown += `# Compatibility metric details\n`;
  Object.entries(details).forEach(([category, stats]) => {
    markdown += `## ${category}\n`;
    markdown += `- total: ${stats.total}\n`;
    markdown += `- passed: ${stats.total - stats.failed}\n`;
    markdown += `- passed ratio: ${
      (((stats.total - stats.failed) / stats.total) * 100).toFixed(2)
    }%\n`;
  });
  fs.writeFileSync(
    path.resolve(import.meta.dirname, './stats/stats.md'),
    markdown,
  );
}

type Summary = {
  markdown: string;
  stats: Stats;
};

function getSummaryMarkdownAndStats(
  diffList: Array<{
    diffResult: UnwrapPromise<ReturnType<typeof diffCase>>;
    name: string;
  }>,
  snapshotCategory: string,
): Summary {
  let bypassList = [];
  let failedList = [];
  let passList = [];
  for (let diff of diffList) {
    if (diff.diffResult === 'bypass') {
      bypassList.push(diff);
    } else if (diff.diffResult === 'same') {
      passList.push(diff);
    } else {
      failedList.push(diff);
    }
  }
  let markdown = `# Failed Cases\n`;
  for (let diff of failedList) {
    let testDir = path.join(esbuildTestDir, snapshotCategory, diff.name);
    let relativePath = path.relative(
      path.join(import.meta.dirname, 'summary'),
      testDir,
    );
    const posixPath = relativePath.replaceAll('\\', '/');
    if (diff.diffResult === 'missing') {
      markdown += `## ${diff.name}\n`;
      markdown += `  missing\n`;
      continue;
    }
    if (diff.diffResult !== 'same') {
      markdown += `## [${diff.name}](${posixPath}/diff.md)\n`;
      markdown += `  diff\n`;
    }
  }

  markdown += `# Passed Cases\n`;
  for (let diff of passList) {
    let testDir = path.join(esbuildTestDir, snapshotCategory, diff.name);
    let relativePath = path.relative(
      path.join(import.meta.dirname, 'summary'),
      testDir,
    );
    const posixPath = relativePath.replaceAll('\\', '/');
    markdown += `## [${diff.name}](${posixPath})\n`;
  }

  markdown += `# Bypassed Cases\n`;
  for (let diff of bypassList) {
    let testDir = path.join(esbuildTestDir, snapshotCategory, diff.name);
    let relativePath = path.relative(
      path.join(import.meta.dirname, 'summary'),
      testDir,
    );
    const posixPath = relativePath.replaceAll('\\', '/');
    markdown += `## [${diff.name}](${posixPath}/bypass.md)\n`;
  }

  return {
    markdown,
    stats: {
      pass: passList.length,
      bypass: bypassList.length,
      failed: failedList.length,
      total: diffList.length,
    },
  };
}

function updateBypassOrDiffMarkdown(
  markdownPath: string,
  diffResult: UnwrapPromise<ReturnType<typeof diffCase>>,
) {
  let bypassContent = '';
  if (fs.existsSync(markdownPath)) {
    bypassContent = fs.readFileSync(markdownPath, 'utf-8');
  }

  let res = /# Diff/.exec(bypassContent);
  if (res) {
    bypassContent = bypassContent.slice(0, res.index);
  }
  let diffMarkdown = getDiffMarkdown(diffResult);
  bypassContent = bypassContent.trimEnd();
  bypassContent += '\n# Diff\n';
  bypassContent += diffMarkdown;
  fs.writeFileSync(markdownPath, bypassContent.trim());
}
