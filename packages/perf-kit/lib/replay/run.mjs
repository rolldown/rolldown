// `record`, `inspect` and `replay` - field numbers for a build that is not deployed.
// Standalone entry: replay.mjs (perf-kit-replay). The archive folder is the whole
// interface: one agent records, another inspects and replays.
//
// record: loads ONE deployed variant in the user's logged-in Chrome (like
//   `field`), N timing runs + one body run, and writes a replay archive OUTSIDE
//   the repo: every response with safelisted headers, its median server time,
//   a network model fitted from the asset downloads, and HANDOFF.txt.
// inspect: what an archive holds, whether it is complete, how to replay it.
// replay: serves that archive to a throwaway headless Chrome from one local
//   HTTP/2 server for every host. The local dist replaces the deployed files;
//   API responses keep their recorded server time; all bodies share the
//   recorded bandwidth. Alternating pairs, the same reports as `field`.
//   --calibrate first: the replay of the RECORDED build must reproduce the field
//   medians, or the model is wrong and an A/B result means nothing.
// Each command returns a result object (replay.mjs --json prints it).

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

import { launchBrowser, sleep } from '../cdp.mjs';
import { analyzeChunkDiscovery, loadChunkGraph, requestSamples } from '../chunk-discovery.mjs';
import { criticalPath, makeRedactor } from '../field/analysis.mjs';
import {
  attachBrowser,
  contextForOrigin,
  restoreCookies,
  snapshotCookies,
} from '../field/browser.mjs';
import { bodyRun, diagnosticRun, timingRun } from '../field/capture.mjs';
import { loadFieldConfig, validateFieldConfig } from '../field/config.mjs';
import {
  MAX_RETRIES,
  buildDiagnosticReports,
  buildFieldReport,
  buildRuntimeReport,
  msStr,
  printFieldReport,
  slug,
  writeJson,
} from '../field/run.mjs';
import { replayCertificate } from './cert.mjs';
import {
  ARCHIVE_SCHEMA,
  archiveGaps,
  bodyHashes,
  buildArchive,
  calibrate,
  calibrationVerdict,
  isStaticRequest,
  mergeDocument,
  safeHeaders,
  sha256,
} from './model.mjs';
import { chooseCompression, distFile, startReplayServer } from './server.mjs';

const mbit = (bps) => (typeof bps === 'number' ? `${((bps * 8) / 1e6).toFixed(1)} Mbit/s` : 'n/a');
const identity = (v) => v;
const CLI = path.join(path.dirname(fileURLToPath(import.meta.url)), '..', '..', 'replay.mjs');

function insideGitWorkTree(dir) {
  for (let d = path.resolve(dir); ; d = path.dirname(d)) {
    if (fs.existsSync(path.join(d, '.git'))) return d;
    if (path.dirname(d) === d) return null;
  }
}

/** The commands the replaying agent runs next, with absolute paths. */
function replayCommands(archiveDir, build) {
  const dist = build ?? '<the build the site served>';
  return {
    inspect: `node ${CLI} inspect --archive ${archiveDir}`,
    calibrate: `node ${CLI} replay --archive ${archiveDir} --dist recorded=${dist} --calibrate`,
    compare: `node ${CLI} replay --archive ${archiveDir} --dist base=${dist} --dist candidate=<local dist>`,
  };
}

// --- record ------------------------------------------------------------------------

const RECORD_OPTIONS = {
  config: { type: 'string' },
  variant: { type: 'string' },
  url: { type: 'string' },
  cookie: { type: 'string', multiple: true },
  block: { type: 'string', multiple: true },
  gate: { type: 'string' },
  'ignore-lcp': { type: 'string' },
  redact: { type: 'string', multiple: true },
  build: { type: 'string' },
  runs: { type: 'string', default: '6' },
  pause: { type: 'string', default: '3000' },
  settle: { type: 'string', default: '3000' },
  timeout: { type: 'string', default: '30000' },
  out: { type: 'string' },
  'cdp-url': { type: 'string' },
  'cdp-port': { type: 'string' },
};

/**
 * What to record: a variant of a field config (--config [--variant]), or a page
 * described by flags alone (--url, --cookie name=value, --block, --gate,
 * --ignore-lcp, --redact), so a recording agent needs no file. --build names the
 * local copy of the build the site serves (calibration and HTML check).
 * Errors name a cookie, never its value.
 */
export function recordTarget(opts, baseDir = process.cwd()) {
  let config;
  let variant;
  if (opts.config) {
    config = loadFieldConfig(opts.config);
    variant = opts.variant ? config.variants[opts.variant] : Object.values(config.variants)[0];
    if (!variant) throw new Error(`no variant "${opts.variant}" in the config`);
  } else {
    if (!opts.url)
      throw new Error(
        'record needs --url <page> (with --cookie/--block/--gate as needed) or --config <file>',
      );
    const name = opts.variant ?? 'recorded';
    const cookies = (opts.cookie ?? []).map((pair) => {
      const eq = pair.indexOf('=');
      if (eq < 1) throw new Error(`--cookie needs name=value (got "${pair.split('=')[0]}")`);
      return { name: pair.slice(0, eq), value: pair.slice(eq + 1) };
    });
    config = validateFieldConfig(
      {
        url: opts.url,
        variants: { [name]: { cookies } },
        block: opts.block ?? [],
        ignoreLcp: opts['ignore-lcp'] ?? null,
        gate: opts.gate ?? null,
        redact: opts.redact ?? [],
      },
      baseDir,
    );
    variant = config.variants[name];
  }
  if (opts.build) variant = { ...variant, build: path.resolve(baseDir, opts.build) };
  return { config, variant };
}

export async function cmdRecord(argv) {
  const opts = parseArgs({ args: argv, options: RECORD_OPTIONS, allowPositionals: true }).values;
  const { config, variant } = recordTarget(opts);
  const outDir = path.resolve(
    opts.out ?? path.join(os.tmpdir(), 'perf-kit-replay', `${slug(config.url)}-${variant.name}`),
  );
  const repo = insideGitWorkTree(outDir);
  if (repo)
    throw new Error(
      `refusing to write a replay archive inside a git work tree (${repo}): it holds real responses`,
    );
  const runs = Number(opts.runs);
  const timing = { settleMs: Number(opts.settle), timeoutMs: Number(opts.timeout), throttle: null };
  const pause = Number(opts.pause);
  const redact = makeRedactor(config.redact);

  console.log(
    `record: ${redact(variant.url)}  variant: ${variant.name}  runs: ${runs} + 1 body run`,
  );
  const browser = await attachBrowser({ cdpUrl: opts['cdp-url'], cdpPort: opts['cdp-port'] });
  const { cdp } = browser;
  const chrome = await cdp
    .send('Browser.getVersion')
    .then((v) => v.product)
    .catch(() => null);
  const ctx = await contextForOrigin(cdp, config.origin);
  if (!ctx.found) console.log(`note: no open tab on ${config.origin}; using the default profile.`);
  const names = [...new Set(variant.cookies.map((c) => c.name))];
  const saved = await snapshotCookies(cdp, {
    browserContextId: ctx.browserContextId,
    origin: config.origin,
    names,
  });
  let restored = false;
  const restore = async () => {
    if (restored) return;
    restored = true;
    await restoreCookies(cdp, {
      browserContextId: ctx.browserContextId,
      origin: config.origin,
      saved,
      configured: variant.cookies,
    });
  };
  const onSigint = async () => {
    await restore().catch(() => {});
    process.exit(130);
  };
  process.once('SIGINT', onSigint);
  // Timing runs keep raw URLs (identity redactor): the replay matches on them.
  const common = {
    config,
    variant,
    context: ctx.browserContextId,
    saved,
    redact: identity,
    ...timing,
  };

  try {
    const timed = [];
    for (let i = 0; i < runs; i++) {
      for (let attempt = 0; attempt <= MAX_RETRIES; attempt++) {
        const run = await timingRun(cdp, common);
        if (run.invalid) {
          console.log(
            `  run ${i + 1} INVALID: ${run.invalid}${attempt < MAX_RETRIES ? ' - retrying' : ''}`,
          );
          await sleep(pause);
          continue;
        }
        const cp = criticalPath(run.requests, run.sample.lcp, { pattern: config.gate });
        timed.push({
          requests: run.requests,
          lcp: run.sample.lcp,
          fcp: run.sample.fcp ?? null,
          lcpElement: run.sample.lcpElement ?? null,
          criticalPath: cp,
          path: run.path,
          connection: run.connection ?? null,
        });
        const b = cp.breakdown;
        console.log(
          `  run ${i + 1} LCP ${msStr(run.sample.lcp).padStart(7)}` +
            (b
              ? `  gate start ${msStr(b.beforeGateMs)}  ttfb ${msStr(b.ttfbMs)}  after ${msStr(b.afterGateMs)}`
              : `  gate: ${cp.reason}`),
        );
        await sleep(pause);
        break;
      }
    }
    if (!timed.length) throw new Error('no valid timing run');

    let body = null;
    for (let attempt = 0; attempt <= MAX_RETRIES && !body; attempt++) {
      const run = await bodyRun(cdp, common);
      if (run.invalid) {
        console.log(
          `  body run INVALID: ${run.invalid}${attempt < MAX_RETRIES ? ' - retrying' : ''}`,
        );
        await sleep(pause);
        continue;
      }
      body = run;
    }
    if (!body) throw new Error('no valid body run');

    fs.rmSync(outDir, { recursive: true, force: true });
    fs.mkdirSync(path.join(outDir, 'bodies'), { recursive: true, mode: 0o700 });
    fs.chmodSync(outDir, 0o700);
    let bodyBytes = 0;
    const rows = body.rows.map(({ body: data, postData, headers, ...r }) => {
      let bodyRef = null;
      if (data?.length) {
        bodyRef = sha256(data);
        const file = path.join(outDir, 'bodies', bodyRef);
        if (!fs.existsSync(file)) {
          fs.writeFileSync(file, data, { mode: 0o600 });
          bodyBytes += data.length;
        }
      }
      const { bodyHash, bodyNormHash } = bodyHashes(postData);
      return {
        ...r,
        headers: safeHeaders(headers),
        bodyRef,
        bodyBytes: data?.length ?? 0,
        reqBodyHash: bodyHash,
        reqBodyNormHash: bodyNormHash,
      };
    });
    const archive = buildArchive({
      config,
      variant,
      timing: timed,
      bodyRun: { rows, page: body.page },
    });
    archive.recordedWith = {
      tool: 'perf-kit replay',
      archiveSchema: ARCHIVE_SCHEMA,
      chrome,
      node: process.version,
      cookieNames: names,
      timingRuns: timed.length,
    };
    fs.writeFileSync(path.join(outDir, 'archive.json'), `${JSON.stringify(archive, null, 2)}\n`, {
      mode: 0o600,
    });
    // The field waterfalls, for a calibration that fails: what did the page wait for?
    fs.writeFileSync(
      path.join(outDir, 'timing.json'),
      `${JSON.stringify({ runs: timed }, null, 2)}\n`,
      { mode: 0o600 },
    );
    const commands = replayCommands(outDir, variant.build);
    fs.writeFileSync(path.join(outDir, 'HANDOFF.txt'), handoffText(archive, { redact, commands }), {
      mode: 0o600,
    });
    printArchiveSummary(archive, outDir, redact);
    const gaps = findGaps(archive, timed, archive.build, redact);
    printGaps(gaps);
    return {
      archive: outDir,
      handoff: path.join(outDir, 'HANDOFF.txt'),
      complete: gaps.length === 0,
      gaps,
      ...archiveFacts(archive, outDir, redact),
      bodyBytesWritten: bodyBytes,
      next: commands,
    };
  } finally {
    process.off('SIGINT', onSigint);
    await restore().catch((err) => console.error(`WARNING: cookie restore failed: ${err.message}`));
    if (names.length) console.log(`cookies restored: ${names.join(', ')} (values not printed)`);
    await browser.close().catch(() => {});
  }
}

function handoffText(archive, { redact, commands }) {
  const f = archive.field;
  const n = archive.network;
  return [
    'perf-kit replay archive',
    '',
    `recorded: ${redact(archive.url)}  variant ${archive.variant}  ${new Date(archive.recordedAtMs).toISOString()}`,
    `          ${f.runs} timing runs, ${archive.recordedWith?.chrome ?? 'Chrome'}, Chrome rated the network ${n.effectiveType ?? 'n/a'}`,
    `field medians: LCP ${msStr(f.lcpMs)}, document end ${msStr(f.documentEndMs)}, gate start ${msStr(f.beforeGateMs)},` +
      ` gate ttfb ${msStr(f.ttfbMs)}, after gate ${msStr(f.afterGateMs)}`,
    `local copy of the recorded build: ${archive.build ?? 'not given (record --build <dir>)'}`,
    '',
    'This folder holds REAL responses of the recording account. Keep it on this machine.',
    'Never copy it into a repo, a ticket, a merge request or a chat.',
    '',
    'Next (in this order; --json prints one result object on stdout):',
    `  ${commands.inspect}`,
    `  ${commands.calibrate}`,
    `  ${commands.compare}`,
    'Trust an A/B only after the calibration passes. The calibration "verdict" says what to do:',
    '  pass - an A/B is usable; record-again - record again (longer --settle);',
    '  model-error - the archive is complete, a new recording does not fix it: report the rows.',
    '',
  ].join('\n');
}

const archiveBodyBytes = (archiveDir) => {
  try {
    const dir = path.join(archiveDir, 'bodies');
    return fs.readdirSync(dir).reduce((s, f) => s + fs.statSync(path.join(dir, f)).size, 0);
  } catch {
    return 0;
  }
};

/** The field timing runs of an archive (timing.json), or [] for an archive without them. */
function readTimingRuns(archiveDir) {
  try {
    return JSON.parse(fs.readFileSync(path.join(archiveDir, 'timing.json'), 'utf8')).runs ?? [];
  } catch {
    return [];
  }
}

/**
 * Requests most field runs made that the replay of `build` answers with a 404,
 * as the server routes them: a same-origin GET for a file of the build comes
 * from the build. Without a build, same-origin static files count as served.
 */
function findGaps(archive, runs, build, redact) {
  const dist = build && fs.existsSync(build) ? build : null;
  const inBuild = (r) => {
    if (r.method !== 'GET' || !r.url.startsWith(`${archive.origin}/`)) return false;
    return dist
      ? Boolean(distFile(dist, new URL(r.url).pathname))
      : isStaticRequest(r, archive.origin);
  };
  return archiveGaps(archive, runs, { inBuild }).map((g) => ({ ...g, key: redact(g.key) }));
}

function printGaps(gaps) {
  if (!gaps.length) return;
  console.log(
    `  GAPS: ${gaps.length} request(s) that most field runs made have no recorded response (404 in the replay):`,
  );
  for (const g of gaps.slice(0, 10)) console.log(`    ${g.runs}/${g.of} runs  ${g.key}`);
  console.log('  Record again with a longer --settle (for example 6000).');
}

/**
 * Static chunks that started late before paint (lib/chunk-discovery.mjs), from
 * the CDP waterfalls of field or replay runs and the build's Vite manifest. The
 * threshold is the recorded round trip: a static import that starts more than one
 * RTT after a static ancestor finished was discovered late.
 */
export function discoverChunks(runs, { origin, build, rttMs, redact }) {
  const manifest = build ? path.join(build, '.vite', 'manifest.json') : null;
  const graph = build && fs.existsSync(build) ? loadChunkGraph(build) : null;
  if (!graph && manifest && fs.existsSync(manifest)) {
    // loadChunkGraph refuses a manifest older than a file it names (a stale graph).
    const reason = `${manifest} is older than files of the build; copy a build with mv or cp -p, not cp -R`;
    return {
      fcp: { state: 'unknown', reason, groups: [] },
      lcp: { state: 'unknown', reason, groups: [] },
    };
  }
  return analyzeChunkDiscovery(graph, requestSamples(runs, origin, redact), rttMs);
}

/** The LCP result of discoverChunks, short enough for a --json result. */
const lateChunks = (discovery) => {
  const d = discovery?.lcp;
  if (!d) return null;
  return {
    state: d.state,
    reason: d.reason ?? null,
    rttMs: d.latencyMs ?? null,
    groups: d.groups.map((g) => ({
      parent: g.parent,
      chunks: g.count,
      bytes: g.bytes,
      maxDelayMs: g.maxDelayMs,
      runs: `${g.runsAffected}/${g.measuredRuns}`,
      importChain: g.example.chunks[0]?.importChain ?? [],
      source: g.example.chunks[0]?.facadeModuleId ?? null,
    })),
  };
};

function printLateChunks(discovery, label) {
  const d = lateChunks(discovery);
  if (!d) return;
  if (d.state !== 'open') {
    console.log(
      `  late chunks before LCP (${label}): ${d.state}${d.reason ? ` - ${d.reason}` : ''}`,
    );
    return;
  }
  console.log(
    `  late chunks before LCP (${label}): ${d.groups.length} static import group(s) started more than ${msStr(d.rttMs)} (one RTT) after a static ancestor finished`,
  );
  for (const g of d.groups.slice(0, 4)) {
    console.log(
      `    ${g.parent}: ${g.chunks} chunk(s), ${(g.bytes / 1024).toFixed(1)}KiB, up to ${msStr(g.maxDelayMs)} late (${g.runs} runs)`,
    );
    console.log(
      `      import chain: ${g.importChain.join(' -> ')}${g.source ? `  source: ${g.source}` : ''}`,
    );
  }
  console.log(
    '    Leads, not predicted gains: check whether paint needs them, then test with a replay A/B. Groups overlap; do not add them.',
  );
}

/** The facts an agent needs about an archive, without any response content. */
function archiveFacts(archive, archiveDir, redact) {
  const calls = Object.values(archive.endpoints).reduce((s, e) => s + e.calls.length, 0);
  const n = archive.network;
  return {
    url: redact(archive.url),
    variant: archive.variant,
    recordedAt: new Date(archive.recordedAtMs).toISOString(),
    ageHours: Math.round(((Date.now() - archive.recordedAtMs) / 3_600_000) * 10) / 10,
    recordedWith: archive.recordedWith ?? null,
    build: archive.build,
    gate: archive.gate,
    requestKeys: Object.keys(archive.endpoints).length,
    calls,
    bodyBytes: archiveBodyBytes(archiveDir),
    hosts: archive.hosts,
    field: archive.field,
    network: {
      effectiveType: n.effectiveType ?? null,
      staticTtfbMs: n.staticTtfbMs,
      wireBytes: n.wireBytes ?? 0,
      bandwidthBps: n.bandwidthBps,
      rttMs: n.rttMs,
    },
    page: archive.page
      ? {
          cookieNames: (archive.page.cookieNames ?? []).map(redact),
          localStorageKeys: archive.page.localStorageKeys,
        }
      : null,
  };
}

function printArchiveSummary(archive, archiveDir, redact) {
  const facts = archiveFacts(archive, archiveDir, redact);
  const n = archive.network;
  const f = archive.field;
  console.log(
    `\narchive: ${facts.requestKeys} request keys, ${facts.calls} calls, ${(facts.bodyBytes / 1e6).toFixed(1)} MB of bodies; recorded ${facts.recordedAt} (${facts.ageHours} h ago)`,
  );
  console.log(
    `  volatile query parameters: ${
      Object.keys(archive.volatile).length
        ? Object.entries(archive.volatile)
            .map(([r, p]) => `${redact(r)} [${p.join(',')}]`)
            .join('; ')
        : 'none'
    }`,
  );
  console.log(
    `  hosts: ${archive.hosts.map((h) => `${h.host} ${h.requests}${h.blocked ? ` (${h.blocked} blocked)` : ''}`).join(', ')}`,
  );
  console.log(
    `  document: ttfb ${msStr(archive.document?.ttfbMs)}, download ${msStr(archive.document?.downloadMs)},` +
      ` early share ${archive.document?.earlyFraction ?? 'n/a'}; headers kept: ${Object.keys(archive.document?.headers ?? {}).join(', ') || 'none'}`,
  );
  console.log(
    `  network model: static ttfb ${msStr(n.staticTtfbMs)} on an idle link (${n.estimates.idleLinkSamples} of ${n.estimates.staticSamples} files; all files ${msStr(n.estimates.staticTtfbAllMedianMs)}, p90 ${msStr(n.estimates.staticTtfbP90Ms)}),` +
      ` wire queue ${Math.round((n.wireBytes ?? 0) / 1000)} kB, rtt ${msStr(n.rttMs)}, bandwidth ${mbit(n.bandwidthBps)}` +
      ` (busiest window; fastest single ${mbit(n.estimates.fastestSingleBps)});` +
      ` Chrome rated it ${n.effectiveType ?? 'n/a'} (rtt ${msStr(n.connectionRttMs)})`,
  );
  console.log(
    `  field medians (${f.runs} runs): LCP ${msStr(f.lcpMs)}, document end ${msStr(f.documentEndMs)}, gate start ${msStr(f.beforeGateMs)},` +
      ` gate ttfb ${msStr(f.ttfbMs)}, after gate ${msStr(f.afterGateMs)}  path ${f.paths.map(redact).join(', ')}`,
  );
  if (archive.page) {
    console.log(
      `  page state the replay starts WITHOUT: script-visible cookies ${archive.page.cookieNames?.length ?? 0}` +
        ` (${(archive.page.cookieNames ?? []).map(redact).join(', ') || 'none'}), localStorage keys ${archive.page.localStorageKeys ?? 'n/a'}`,
    );
  }
  if (archive.build && archive.document?.bodyRef) {
    const local = path.join(archive.build, 'index.html');
    if (fs.existsSync(local)) {
      const a = fs.readFileSync(path.join(archiveDir, 'bodies', archive.document.bodyRef), 'utf8');
      const b = fs.readFileSync(local, 'utf8');
      const al = a.split('\n');
      const bl = new Set(b.split('\n'));
      const differing = al.filter((line) => !bl.has(line)).length;
      console.log(
        a === b
          ? `  document HTML: identical to ${local}`
          : `  document HTML: DIFFERS from ${local} (${a.length} vs ${b.length} chars, ${differing} recorded lines not in the local file${/\snonce=/.test(a) ? ', has nonce attributes' : ''}). The replay adds the recorded-only tags to the local file (--document merged).`,
      );
    }
  }
  console.log(`\nfiles: ${archiveDir}`);
  console.log(
    'This archive holds REAL responses from the recording account. Keep it on this machine; delete it with rm -rf when done.',
  );
}

// --- inspect -----------------------------------------------------------------------

/**
 * Problems that make an archive unusable (fatal) or a replay less faithful.
 * The replaying agent checks this before it trusts anything else in the folder.
 */
export function validateArchive(archive, archiveDir) {
  const fatal = [];
  const warnings = [];
  if (archive?.kind !== 'perf-kit-replay') fatal.push('not a replay archive (kind)');
  else if (archive.schemaVersion !== ARCHIVE_SCHEMA) {
    fatal.push(
      `archive schema ${archive.schemaVersion}; this tool reads schema ${ARCHIVE_SCHEMA} - record again`,
    );
  }
  for (const key of ['url', 'origin', 'endpoints', 'network', 'field']) {
    if (archive?.[key] == null) fatal.push(`missing "${key}"`);
  }
  if (fatal.length) return { fatal, warnings, missingBodies: 0 };
  const refs = new Set(
    [
      archive.document?.bodyRef,
      ...Object.values(archive.endpoints).flatMap((e) => e.calls.map((c) => c.bodyRef)),
    ].filter(Boolean),
  );
  const missingBodies = [...refs].filter(
    (ref) => !fs.existsSync(path.join(archiveDir, 'bodies', ref)),
  ).length;
  if (missingBodies)
    fatal.push(`${missingBodies} of ${refs.size} response bodies are missing from bodies/`);
  if (!archive.document)
    warnings.push('no document recorded: the replay cannot serve the page HTML');
  if (!archive.network.bandwidthBps)
    warnings.push('no bandwidth in the model: downloads are not shaped');
  if (!archive.network.effectiveType)
    warnings.push('no Chrome network rating recorded: Chrome rates loopback as fast (use --ect)');
  if (!archive.build)
    warnings.push(
      'no local copy of the recorded build named: calibration needs --dist recorded=<dir>',
    );
  if (Date.now() - archive.recordedAtMs > 24 * 3_600_000)
    warnings.push('recorded more than 24 h ago: the network and the data may have changed');
  return { fatal, warnings, missingBodies };
}

export function loadArchive(dir) {
  const archiveDir = path.resolve(dir);
  const file = path.join(archiveDir, 'archive.json');
  if (!fs.existsSync(file)) throw new Error(`no replay archive at ${file} (run \`record\` first)`);
  const archive = JSON.parse(fs.readFileSync(file, 'utf8'));
  const check = validateArchive(archive, archiveDir);
  if (check.fatal.length) throw new Error(`${file}: ${check.fatal.join('; ')}`);
  return archive;
}

export async function cmdInspect(argv) {
  const opts = parseArgs({
    args: argv,
    options: { archive: { type: 'string' } },
    allowPositionals: true,
  }).values;
  if (!opts.archive) throw new Error('inspect needs --archive <dir>');
  const archiveDir = path.resolve(opts.archive);
  const file = path.join(archiveDir, 'archive.json');
  if (!fs.existsSync(file)) throw new Error(`no replay archive at ${file}`);
  const archive = JSON.parse(fs.readFileSync(file, 'utf8'));
  const check = validateArchive(archive, archiveDir);
  if (check.fatal.length) {
    console.log(`archive: ${archiveDir}\n  UNUSABLE: ${check.fatal.join('; ')}`);
    return { archive: archiveDir, usable: false, complete: false, ...check, gaps: [] };
  }
  const redact = makeRedactor(archive.redact);
  console.log(`archive: ${archiveDir}  (${redact(archive.url)}, variant ${archive.variant})`);
  printArchiveSummary(archive, archiveDir, redact);
  for (const w of check.warnings) console.log(`  warning: ${w}`);
  const runs = readTimingRuns(archiveDir);
  const gaps = findGaps(archive, runs, archive.build, redact);
  printGaps(gaps);
  const discovery = archive.build
    ? discoverChunks(runs, {
        origin: archive.origin,
        build: archive.build,
        rttMs: archive.network.rttMs,
        redact,
      })
    : {
        lcp: {
          state: 'unknown',
          reason: 'no local copy of the recorded build named (record --build <dir>)',
          groups: [],
        },
      };
  printLateChunks(discovery, 'field runs');
  const next = replayCommands(archiveDir, archive.build);
  console.log(`\nnext:\n  ${next.calibrate}\n  ${next.compare}`);
  return {
    archive: archiveDir,
    usable: true,
    complete: gaps.length === 0,
    ...check,
    gaps,
    lateChunksBeforeLcp: lateChunks(discovery),
    ...archiveFacts(archive, archiveDir, redact),
    next,
  };
}

// --- replay ------------------------------------------------------------------------

const REPLAY_OPTIONS = {
  archive: { type: 'string' },
  dist: { type: 'string', multiple: true },
  runs: { type: 'string', default: '6' },
  diagnose: { type: 'string', default: '0' },
  cut: { type: 'string' },
  calibrate: { type: 'boolean', default: false },
  document: { type: 'string', default: 'merged' },
  bandwidth: { type: 'string' },
  rtt: { type: 'string' },
  'static-ttfb': { type: 'string' },
  wire: { type: 'string' },
  ect: { type: 'string' },
  storage: { type: 'string' },
  pause: { type: 'string', default: '300' },
  settle: { type: 'string', default: '3000' },
  timeout: { type: 'string', default: '30000' },
  out: { type: 'string' },
};

const ECT_NAMES = { 'slow-2g': 'Slow-2G', '2g': '2G', '3g': '3G', '4g': '4G' };

/**
 * Chrome switches that send every host to the replay server and accept its
 * certificate. `--no-proxy-server`: through a system proxy, the proxy would
 * resolve the hosts and the requests would leave the machine. Chrome's own
 * service requests (model downloads, autofill) would reach the replay server
 * too; they are turned off to keep the logs clean.
 * `ect` forces Chrome's network rating: over loopback Chrome rates the network
 * as fast and never holds back low-priority requests, which a slow real
 * network makes it do.
 */
export const replayBrowserArgs = (port, { ect = null } = {}) => [
  `--host-resolver-rules=MAP * 127.0.0.1:${port}, EXCLUDE localhost`,
  '--no-proxy-server',
  '--ignore-certificate-errors',
  '--disable-features=OptimizationHints,OptimizationGuideModelDownloading,OptimizationHintsFetching,AutofillServerCommunication,MediaRouter',
  '--disable-client-side-phishing-detection',
  '--no-pings',
  ...(ect && ECT_NAMES[ect] ? [`--force-effective-connection-type=${ECT_NAMES[ect]}`] : []),
];

function parseDists(values, archive) {
  const list = values?.length ? values : archive.build ? [`recorded=${archive.build}`] : [];
  if (!list.length) throw new Error('replay needs --dist <dir> (or name=<dir>, repeat for an A/B)');
  const used = new Set();
  return list.map((value, i) => {
    const eq = value.indexOf('=');
    let name =
      eq > 0 ? value.slice(0, eq) : i === 0 ? 'base' : `candidate${list.length > 2 ? i : ''}`;
    const dir = path.resolve(eq > 0 ? value.slice(eq + 1) : value);
    if (!fs.existsSync(path.join(dir, 'index.html')))
      throw new Error(`--dist ${dir} has no index.html`);
    while (used.has(name)) name = `${name}-${i}`;
    used.add(name);
    return { name, cookies: [], url: archive.url, build: dir };
  });
}

export async function cmdReplay(argv) {
  const opts = parseArgs({ args: argv, options: REPLAY_OPTIONS, allowPositionals: true }).values;
  if (!opts.archive) throw new Error('replay needs --archive <dir> (written by `record`)');
  const archiveDir = path.resolve(opts.archive);
  const archive = loadArchive(archiveDir);
  const variants = parseDists(opts.dist, archive);
  if (!['merged', 'local', 'recorded'].includes(opts.document))
    throw new Error('--document must be merged, local or recorded');
  const runs = Number(opts.runs);
  const diagnose = Number(opts.diagnose);
  const cut = opts.cut ?? (archive.gate ? 'gate' : 'lcp');
  const pause = Number(opts.pause);
  const timing = { settleMs: Number(opts.settle), timeoutMs: Number(opts.timeout), throttle: null };
  const network = {
    ...archive.network,
    ...(opts.bandwidth ? { bandwidthBps: (Number(opts.bandwidth) * 1e6) / 8 } : {}),
    ...(opts.rtt ? { rttMs: Number(opts.rtt) } : {}),
    ...(opts['static-ttfb']
      ? { staticTtfbMs: Number(opts['static-ttfb']), staticTtfbPoints: null }
      : {}),
    ...(opts.wire ? { wireBytes: Number(opts.wire) * 1000 } : {}),
  };
  const ect = opts.ect === 'off' ? null : (opts.ect ?? archive.network.effectiveType ?? null);
  if (ect && !ECT_NAMES[ect]) throw new Error('--ect must be slow-2g, 2g, 3g, 4g or off');
  // The field runs load the page in a profile that has visited it before. A page
  // that caches data in storage (a boot payload, flags) starts without waiting
  // for it there, so by default the replay is warm when the recorded page had storage.
  const storedKeys = archive.page?.localStorageKeys ?? 0;
  const storage = opts.storage ?? (storedKeys > 0 ? 'warm' : 'empty');
  if (!['warm', 'empty'].includes(storage)) throw new Error('--storage must be warm or empty');
  const compression = chooseCompression(archive.assets, variants[0].build);
  if (!compression.encoding) {
    const enc = archive.assets.find((a) => a.encoding)?.encoding ?? null;
    Object.assign(compression, { encoding: enc, level: null });
  }
  const redact = makeRedactor(archive.redact);
  const config = {
    url: archive.url,
    origin: archive.origin,
    variants: Object.fromEntries(variants.map((v) => [v.name, v])),
    block: archive.block,
    ignoreLcp: archive.ignoreLcp,
    gate: archive.gate,
    redact: archive.redact,
  };
  const outDir = path.resolve(opts.out ?? path.join(archiveDir, 'replay-report'));

  console.log(`replay: ${redact(archive.url)} from ${archiveDir}`);
  console.log(`  dists: ${variants.map((v) => `${v.name}=${v.build}`).join('  ')}`);
  console.log(
    `  network: static ttfb ${network.staticTtfbPoints ? `sampled (idle-link median ${msStr(network.staticTtfbMs)}, p90 ${msStr(network.staticTtfbPoints[18])})` : msStr(network.staticTtfbMs)},` +
      ` wire queue ${Math.round((network.wireBytes ?? 0) / 1000)} kB, rtt ${msStr(network.rttMs)}, bandwidth ${mbit(network.bandwidthBps)} shared;` +
      ` API: recorded server time; document: ${opts.document} HTML after ${msStr(archive.document?.ttfbMs)}` +
      (archive.document?.earlyFraction != null
        ? ` (${Math.round(archive.document.earlyFraction * 100)}% at once, the rest ${msStr(archive.document.downloadMs)} later)`
        : ''),
  );
  console.log(
    `  Chrome network rating: ${ect ?? 'not forced (Chrome rates loopback as fast)'}${opts.ect ? ' (--ect)' : ect ? ' (recorded)' : ''}`,
  );
  console.log(
    `  storage: ${storage}` +
      (opts.storage
        ? ' (--storage)'
        : storage === 'warm'
          ? ` (the recorded page had ${storedKeys} localStorage keys; --storage empty for a first visit)`
          : ' (the recorded page had no localStorage)'),
  );
  console.log(
    `  compression: ${compression.encoding ?? 'none'}${compression.level != null ? ` level ${compression.level}` : ''}` +
      (compression.files
        ? ` (recorded/local size ${compression.recordedOverLocal} over ${compression.files} files)`
        : ''),
  );
  if (opts.document === 'merged' && archive.document?.bodyRef) {
    const recordedHtml = fs.readFileSync(
      path.join(archiveDir, 'bodies', archive.document.bodyRef),
      'utf8',
    );
    const { added } = mergeDocument(
      recordedHtml,
      fs.readFileSync(path.join(variants[0].build, 'index.html'), 'utf8'),
    );
    console.log(
      `  document tags added from the recorded HTML: ${added.length ? added.map((a) => (a.ref ? `<${a.tag} ${a.ref}>` : `<${a.tag}> inline ${a.inlineChars} chars`)).join(', ') : 'none'}`,
    );
  }

  const server = await startReplayServer({
    archive,
    archiveDir,
    tls: replayCertificate(),
    network,
    compression,
    documentSource: opts.document,
  });
  const profileDir = fs.mkdtempSync(path.join(os.tmpdir(), 'perf-kit-replay-chrome-'));
  const browser = await launchBrowser({
    profileDir,
    extraArgs: replayBrowserArgs(server.port, { ect }),
  });
  const { cdp } = browser;
  await cdp.send('Security.setIgnoreCertificateErrors', { ignore: true }).catch(() => {});
  const common = { config, saved: new Map(), redact, ...timing };

  // A fresh browser context per load: no storage, cookies or connections carry
  // over. Both builds of a pair get the same seed: the same sampled network.
  // `warm` first loads the page once, unmeasured, in that context: the build
  // writes its own storage, as on an earlier visit, and the measured load reads
  // it. Every load keeps the HTTP cache off and bypasses service workers, as the
  // field runs do, so only storage and cookies carry over.
  const load = async (variant, kind, seed = 0) => {
    const { browserContextId } = await cdp.send('Target.createBrowserContext');
    try {
      if (storage === 'warm') {
        server.beginRun(variant.build, { seed });
        await timingRun(cdp, { ...common, variant, context: browserContextId });
      }
      server.beginRun(variant.build, { seed });
      const run =
        kind === 'diagnostic'
          ? await diagnosticRun(cdp, { ...common, variant, context: browserContextId, cut })
          : await timingRun(cdp, { ...common, variant, context: browserContextId });
      return { run, log: server.takeLog() };
    } finally {
      await cdp.send('Target.disposeBrowserContext', { browserContextId }).catch(() => {});
    }
  };

  try {
    console.log('\nwarm-up (one unmeasured load per dist; fills the compression cache)');
    for (const v of variants) await load(v, 'timing');

    const timed = [];
    const invalid = [];
    const logs = new Map(variants.map((v) => [v.name, []]));
    for (let i = 0; i < runs; i++) {
      const order = i % 2 ? [...variants].reverse() : variants;
      for (const variant of order) {
        for (let attempt = 0; attempt <= MAX_RETRIES; attempt++) {
          const { run, log } = await load(variant, 'timing', i + 1);
          if (run.invalid) {
            invalid.push({ pair: i + 1, variant: variant.name, reason: run.invalid });
            console.log(
              `  pair ${i + 1} ${variant.name.padEnd(10)} INVALID: ${run.invalid}${attempt < MAX_RETRIES ? ' - retrying' : ''}`,
            );
            continue;
          }
          const cp = criticalPath(run.requests, run.sample.lcp, { pattern: config.gate });
          timed.push({ pair: i + 1, variant: variant.name, ...run, criticalPath: cp });
          logs.get(variant.name).push(...log);
          const b = cp.breakdown;
          const misses = log.filter((e) => e.source === 'miss').length;
          console.log(
            `  pair ${i + 1} ${variant.name.padEnd(10)} LCP ${msStr(run.sample.lcp).padStart(7)}` +
              (b
                ? `  doc end ${msStr(b.documentEndMs)}  gate start ${msStr(b.beforeGateMs)}  ttfb ${msStr(b.ttfbMs)}  after ${msStr(b.afterGateMs)}`
                : `  gate: ${cp.reason}`) +
              (misses ? `  ${misses} unrecorded` : ''),
          );
          await sleep(pause);
          break;
        }
      }
    }

    const diagnostics = new Map();
    for (const variant of diagnose > 0 ? variants : []) {
      const { run } = await load(variant, 'diagnostic');
      if (run.invalid) console.log(`  ${variant.name} diagnostic run INVALID: ${run.invalid}`);
      else diagnostics.set(variant.name, run);
    }

    const summaries = {};
    const replayInfo = {};
    for (const variant of variants) {
      const mine = timed.filter((t) => t.variant === variant.name);
      const runtime = buildRuntimeReport({
        variant,
        mine,
        invalid,
        redact,
        throttle: null,
      });
      runtime.mode = 'replay';
      if (runtime.criticalPath) delete runtime.criticalPath.medianRunRequests;
      replayInfo[variant.name] = summarizeLog(
        logs.get(variant.name),
        mine,
        redact,
        archive.field.paths.map(redact),
      );
      runtime.replay = replayInfo[variant.name];
      runtime.chunkDiscovery = discoverChunks(
        mine.map((t) => ({ requests: t.requests, lcp: t.sample.lcp, fcp: t.sample.fcp ?? null })),
        { origin: archive.origin, build: variant.build, rttMs: network.rttMs, redact },
      );
      writeJson(path.join(outDir, variant.name, 'runtime-metrics.json'), runtime);
      let coverage = null;
      let profile = null;
      const diag = diagnostics.get(variant.name);
      if (diag) {
        ({ coverage, profile } = buildDiagnosticReports({ variant, diag, config, redact }));
        writeJson(path.join(outDir, variant.name, 'coverage.json'), coverage);
        writeJson(path.join(outDir, variant.name, 'profile.json'), profile);
      }
      summaries[variant.name] = { runtime, coverage, profile };
    }
    const report = buildFieldReport({
      variants,
      timed,
      invalid,
      summaries,
      config,
      redact,
      cut,
      throttle: null,
    });
    report.mode = 'replay';
    report.replay = {
      archive: archiveDir,
      recordedVariant: archive.variant,
      network,
      effectiveType: ect,
      storage,
      compression,
      document: opts.document,
      variants: replayInfo,
    };
    report.limits = [
      'a model of one recorded network and one recorded server, not the real ones: run --calibrate before trusting an A/B',
      'API responses are the recorded ones; a request the recording never saw gets a 404 (see replay.variants.*.unrecorded)',
      storage === 'warm'
        ? "each measured load follows one unmeasured load of the same build in the same browser context: storage the build wrote carries over, the recorded profile's own storage does not"
        : 'each load starts with empty storage and no cookies; the recorded page had both (see the archive "page" field)',
      ...report.limits.slice(1),
    ];
    if (opts.calibrate) {
      const first = summaries[variants[0].name].runtime;
      const calibration = calibrate(archive.field, {
        lcpMs: first.metrics['runtime.lcp_ms'],
        ...first.criticalPath?.medians,
      });
      const gaps = findGaps(archive, readTimingRuns(archiveDir), variants[0].build, redact);
      report.calibration = { ...calibration, verdict: calibrationVerdict(calibration, gaps), gaps };
    }
    writeJson(path.join(outDir, 'replay-report.json'), report);
    printFieldReport(report, summaries, config);
    printReplaySection(report, variants, summaries);
    console.log(`\nfiles: ${outDir}`);
    return {
      report: path.join(outDir, 'replay-report.json'),
      archive: archiveDir,
      effectiveType: ect,
      storage,
      network: {
        staticTtfbMs: network.staticTtfbMs,
        wireBytes: network.wireBytes ?? 0,
        bandwidthBps: network.bandwidthBps,
      },
      variants: Object.fromEntries(
        variants.map((v) => {
          const s = report.variants[v.name];
          const info = replayInfo[v.name];
          return [
            v.name,
            {
              dist: v.build,
              validRuns: s.validRuns,
              invalidRuns: s.invalidRuns,
              lcpMs: s.lcpMs,
              medians: s.criticalPath?.medians ?? null,
              unrecordedPageRequests: info.unrecorded.filter((u) => u.byPage),
              pathMatchesField: info.pathMatchesField,
              lateChunksBeforeLcp: lateChunks(summaries[v.name].runtime.chunkDiscovery),
            },
          ];
        }),
      ),
      comparisons: report.comparisons,
      calibration: report.calibration ?? null,
    };
  } finally {
    await browser.close().catch(() => {});
    await server.close().catch(() => {});
    fs.rmSync(profileDir, { recursive: true, force: true });
  }
}

/** What the server did for one variant: match levels, unrecorded requests, the landing path. */
export function summarizeLog(log, mine, redact, fieldPaths = []) {
  const levels = {};
  for (const e of log) {
    const k = `${e.source}:${e.level}`;
    levels[k] = (levels[k] ?? 0) + 1;
  }
  // A request of the page: started by its HTML or scripts. The browser's own
  // requests (favicon, navigation) have the initiator type "other".
  const pageUrls = new Set(
    mine.flatMap((t) =>
      t.requests.filter((r) => r.initiator?.type && r.initiator.type !== 'other').map((r) => r.url),
    ),
  );
  const unrecorded = new Map();
  for (const e of log.filter((x) => x.source === 'miss')) {
    const url = redact(e.url);
    const key = `${e.method} ${url}`;
    const row = unrecorded.get(key) ?? {
      method: e.method,
      url,
      count: 0,
      byPage: pageUrls.has(url),
    };
    row.count++;
    unrecorded.set(key, row);
  }
  const paths = [...new Set(mine.map((t) => t.path))];
  return {
    loads: mine.length,
    levels,
    unrecorded: [...unrecorded.values()].sort(
      (a, b) => Number(b.byPage) - Number(a.byPage) || b.count - a.count,
    ),
    paths,
    pathMatchesField: fieldPaths.length ? paths.every((p) => fieldPaths.includes(p)) : null,
  };
}

function printReplaySection(report, variants, summaries) {
  const r = report.replay;
  for (const v of variants) {
    const info = r.variants[v.name];
    const lv = Object.entries(info.levels)
      .sort((a, b) => b[1] - a[1])
      .map(([k, n]) => `${k} ${n}`)
      .join(', ');
    console.log(`\n${v.name}: served ${lv}`);
    if (info.pathMatchesField === false)
      console.log(
        `  WARNING: the replay landed on ${info.paths.join(', ')}, not on the recorded path`,
      );
    const byPage = info.unrecorded.filter((u) => u.byPage);
    if (byPage.length) {
      console.log(
        `  UNRECORDED requests of the page (404 in the replay - the result may not hold):`,
      );
      for (const u of byPage.slice(0, 10)) console.log(`    ${u.count}x ${u.method} ${u.url}`);
    }
    const other = info.unrecorded.length - byPage.length;
    if (other)
      console.log(
        `  ${other} unrecorded request kinds that the page did not start (favicon, browser-internal)`,
      );
    printLateChunks(summaries[v.name].runtime.chunkDiscovery, `${v.name} replay runs`);
  }
  if (report.calibration) {
    const c = report.calibration;
    console.log(
      `\ncalibration: replay of ${variants[0].name} vs the recorded field medians (${c.pass ? 'PASS' : 'FAIL'})`,
    );
    for (const row of c.rows) {
      if (row.pass === null) {
        console.log(
          `  ${row.label.padEnd(13)} field ${msStr(row.field).padStart(7)}  replay ${msStr(row.replay).padStart(7)}  (not compared)`,
        );
        continue;
      }
      console.log(
        `  ${row.label.padEnd(13)} field ${msStr(row.field).padStart(7)}  replay ${msStr(row.replay).padStart(7)}` +
          `  ${row.deltaMs > 0 ? '+' : ''}${row.deltaMs}ms (tolerance ${row.toleranceMs}ms) ${row.pass ? 'ok' : 'OFF'}`,
      );
    }
    const verdicts = {
      pass: 'PASS - every row is within tolerance and the archive is complete; an A/B on this archive is usable',
      'record-again':
        'RECORD AGAIN - the archive misses responses that most field runs got (see the gaps)',
      'model-error':
        'MODEL ERROR - the archive is complete, so a new recording does not fix the rows that are OFF',
    };
    console.log(`verdict: ${verdicts[c.verdict]}`);
    printGaps(c.gaps);
  }
}
