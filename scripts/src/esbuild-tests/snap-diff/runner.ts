import * as fs from 'node:fs';
import * as path from 'node:path';
import {
  failedReasons,
  ignoreReasons,
  notSupportedReasons,
} from '../reasons.js';
import { diffCase } from './diff.js';
import {
  ensureSnapshot,
  SNAPSHOT_FILES,
  type SnapshotFileName,
} from './download-snapshots.js';
import { parseEsbuildSnap, parseRolldownSnap } from './snap-parser.js';
import type { DebugConfig, UnwrapPromise } from './types.js';
const esbuildTestDir = path.join(
  import.meta.dirname,
  '../../../../crates/rolldown/tests/esbuild',
);

function getTestCaseKey(category: string, name: string): string {
  return `${category}/${name}`;
}

function getActualTestPath(
  baseDir: string,
  category: string,
  snapName: string,
): { path: string; isSkipped: boolean } {
  const normalPath = path.join(baseDir, category, snapName);
  const skippedPath = path.join(baseDir, category, '.' + snapName);
  if (fs.existsSync(path.join(skippedPath, '_config.json'))) {
    return { path: skippedPath, isSkipped: true };
  }
  return { path: normalPath, isSkipped: false };
}

async function getEsbuildSnapFile(): Promise<
  Array<{ normalizedName: string; content: string }>
> {
  const ret: Array<{ normalizedName: string; content: string }> = [];

  for (const filename of SNAPSHOT_FILES) {
    const name = path.parse(filename).name;
    const [_, ...rest] = name.split('_');
    const normalizedName = rest.join('_');
    const content = await ensureSnapshot(filename as SnapshotFileName);
    ret.push({ normalizedName, content });
  }

  return ret;
}
type AggregateStats = {
  stats: Stats;
  details: Record<string, Stats>;
};

type Stats = {
  pass: number;
  failed: number;
  ignored: number;
  total: number;
};
export async function run(debugConfig: DebugConfig) {
  let aggregatedStats: AggregateStats = {
    stats: {
      pass: 0,
      failed: 0,
      ignored: 0,
      total: 0,
    },
    details: {},
  };

  const undocumentedSkippedTests: string[] = [];
  const processedFailedTests = new Set<string>();
  let snapfileList = await getEsbuildSnapFile();
  // esbuild snapshot_x.txt
  for (let snapFile of snapfileList) {
    if (debugConfig?.debug) {
      console.log('category:', snapFile.normalizedName);
    }
    let { normalizedName: snapCategory, content } = snapFile;

    // Skip if category directory doesn't exist
    const categoryDir = path.join(esbuildTestDir, snapCategory);
    if (!fs.existsSync(categoryDir)) {
      continue;
    }

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

      const { path: rolldownTestPath, isSkipped } = getActualTestPath(
        esbuildTestDir,
        snapCategory,
        snap.name,
      );
      const testKey = getTestCaseKey(snapCategory, snap.name);
      const isInFailedReasons = testKey in failedReasons;
      const isInIgnoreReasons = testKey in ignoreReasons;
      const isInNotSupportedReasons = testKey in notSupportedReasons;

      // Validate that skipped tests have documented reasons
      if (isSkipped) {
        const hasDocumentedReason = isInIgnoreReasons ||
          isInNotSupportedReasons || isInFailedReasons;
        if (!hasDocumentedReason) {
          undocumentedSkippedTests.push(testKey);
        }
      }

      let rolldownSnap = getRolldownSnap(rolldownTestPath);
      let parsedRolldownSnap = parseRolldownSnap(rolldownSnap);
      let diffResult = await diffCase(
        snap,
        parsedRolldownSnap,
        rolldownTestPath,
        debugConfig,
      );

      // Only generate diff.md if test is in failedReasons
      let diffMarkdownPath = path.join(rolldownTestPath, 'diff.md');
      if (isInFailedReasons && typeof diffResult === 'object') {
        processedFailedTests.add(testKey);
        updateDiffMarkdown(diffMarkdownPath, diffResult);
      } else if (fs.existsSync(diffMarkdownPath)) {
        fs.rmSync(diffMarkdownPath, {});
      }

      diffList.push({
        diffResult,
        name: snap.name,
        isIgnored: isInIgnoreReasons,
        isFailed: isInFailedReasons,
        isNotSupported: isInNotSupportedReasons,
        reason: isInFailedReasons
          ? failedReasons[testKey]
          : isInIgnoreReasons
          ? ignoreReasons[testKey]
          : isInNotSupportedReasons
          ? notSupportedReasons[testKey]
          : undefined,
      });
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
    aggregatedStats.stats.ignored += summary.stats.ignored;
    aggregatedStats.stats.failed += summary.stats.failed;
  }

  if (undocumentedSkippedTests.length > 0) {
    throw new Error(
      `The following skipped tests (with '.' prefix) have no documented reason in reasons.ts:\n` +
        undocumentedSkippedTests.map((t) => `  - ${t}`).join('\n') +
        `\n\nPlease add them to ignoreReasons, notSupportedReasons, or failedReasons in scripts/src/esbuild-tests/reasons.ts`,
    );
  }

  if (!debugConfig.caseNames || debugConfig.caseNames.length === 0) {
    const missingFailedTests = Object.keys(failedReasons).filter(
      (testKey) => !processedFailedTests.has(testKey),
    );
    if (missingFailedTests.length > 0) {
      throw new Error(
        `The following tests are in failedReasons but were not processed or did not have a diff.md generated:\n` +
          missingFailedTests.map((t) => `  - ${t}`).join('\n') +
          `\n\nThis usually means the test case does not exist or now passes. Please verify the test cases exist and update the failedReasons in scripts/src/esbuild-tests/reasons.ts if needed.`,
      );
    }
  }

  generateStatsMarkdown(aggregatedStats);
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
) {
  const { stats, details } = aggregateStats;
  let markdown = '';

  const unsupportedCaseCount = Object.keys(notSupportedReasons).length;

  // Exclude ignored tests from ratio calculation
  const totalForRatio = stats.total - (stats.ignored - unsupportedCaseCount);
  const passed = stats.pass;

  markdown += `# Compatibility metric\n`;
  markdown += `- total: ${stats.total}\n`;
  markdown += `- ignored: ${stats.ignored}\n`;
  markdown += `- passed: ${passed}\n`;
  markdown += `- passed ratio: ${
    ((passed / totalForRatio) * 100).toFixed(2)
  }%\n`;

  let totalWithoutNotSupport = totalForRatio - unsupportedCaseCount;
  markdown += `# Compatibility metric without not supported case\n`;
  markdown += `- total: ${totalWithoutNotSupport}\n`;
  markdown += `- passed: ${passed}\n`;
  markdown += `- passed ratio: ${
    ((passed / totalWithoutNotSupport) * 100).toFixed(2)
  }%\n`;

  markdown += `# Compatibility metric details\n`;
  Object.entries(details).forEach(([category, categoryStats]) => {
    const categoryTotalForRatio = categoryStats.total - categoryStats.ignored;
    markdown += `## ${category}\n`;
    markdown += `- total: ${categoryStats.total}\n`;
    markdown += `- ignored: ${categoryStats.ignored}\n`;
    markdown += `- passed: ${categoryStats.pass}\n`;
    markdown += `- passed ratio: ${
      ((categoryStats.pass / categoryTotalForRatio) * 100).toFixed(2)
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
    isIgnored: boolean;
    isFailed: boolean;
    isNotSupported: boolean;
    reason: string | undefined;
  }>,
  snapshotCategory: string,
): Summary {
  let failedList = [];
  let passList = [];
  let ignoredList = [];
  let notSupportedList = [];
  for (let diff of diffList) {
    if (diff.isNotSupported) {
      notSupportedList.push(diff);
    } else if (diff.isIgnored) {
      ignoredList.push(diff);
    } else if (diff.isFailed) {
      failedList.push(diff);
    } else {
      passList.push(diff);
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
    markdown += `## [${diff.name}](${posixPath}/diff.md)\n`;
    markdown += `  ${diff.reason || 'unknown reason'}\n`;
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

  markdown += `# Ignored Cases\n`;
  for (let diff of ignoredList) {
    let testDir = path.join(esbuildTestDir, snapshotCategory, diff.name);
    let relativePath = path.relative(
      path.join(import.meta.dirname, 'summary'),
      testDir,
    );
    const posixPath = relativePath.replaceAll('\\', '/');
    markdown += `## [${diff.name}](${posixPath})\n`;
    markdown += `  ${diff.reason || 'unknown reason'}\n`;
  }

  markdown += `# Ignored Cases (not supported)\n`;
  for (let diff of notSupportedList) {
    let testDir = path.join(esbuildTestDir, snapshotCategory, diff.name);
    let relativePath = path.relative(
      path.join(import.meta.dirname, 'summary'),
      testDir,
    );
    const posixPath = relativePath.replaceAll('\\', '/');
    markdown += `## [${diff.name}](${posixPath})\n`;
    markdown += `  ${diff.reason || 'unknown reason'}\n`;
  }

  return {
    markdown,
    stats: {
      pass: passList.length,
      failed: failedList.length,
      ignored: ignoredList.length + notSupportedList.length,
      total: diffList.length,
    },
  };
}

function updateDiffMarkdown(
  markdownPath: string,
  diffResult: UnwrapPromise<ReturnType<typeof diffCase>>,
) {
  let diffMarkdown = getDiffMarkdown(diffResult);
  fs.writeFileSync(markdownPath, diffMarkdown.trim());
}
