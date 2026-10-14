#!/usr/bin/env node
// perf-kit-replay - record a deployed page once, replay it against local builds.
// Standalone: no harness target, no lab state. The archive folder is the whole
// interface between the agent that records and the agent that replays.

import { cmdInspect, cmdRecord, cmdReplay } from './lib/replay/run.mjs';

const HELP = `perf-kit-replay - field-like load numbers for builds that are not deployed

  record   load a deployed page in the user's logged-in Chrome and write an archive
  inspect  what an archive holds, whether it is complete, the next commands
  replay   serve an archive to headless Chrome with local builds in place of the deployed files

TWO AGENTS
  recording agent (needs the user's Chrome with chrome://inspect/#remote-debugging on):
    node replay.mjs record --url https://app.example.com/ [--cookie name=value]
        [--block analytics.example.com] [--gate /api/items] [--ignore-lcp '#splash']
        [--build <local copy of the served build>] --json
    -> returns { archive, handoff, complete, gaps, next, ... }. complete:false -> record
       again with a longer --settle. Hand over the "archive" path only.
  replaying agent (no browser login needed):
    node replay.mjs inspect --archive <dir> --json         usable? complete? warnings? recorded when?
                                                           lateChunksBeforeLcp: late static chunks in the field runs
    node replay.mjs replay --archive <dir> --dist recorded=<served build> --calibrate --json --exit-code
    node replay.mjs replay --archive <dir> --dist base=<dir> --dist candidate=<dir> --json
  The calibration verdict says what to do next:
    pass           an A/B on this archive is usable
    record-again   the archive misses responses most field runs got: record again
    model-error    the archive is complete; a new recording does not fix it: report the rows
  Trust an A/B only after the calibration passes. HANDOFF.txt in the archive repeats this.

record
  --url <page> --cookie name=value (repeat) --block <substring> (repeat) --gate <substring>
  --ignore-lcp <selector> --redact <regex> (repeat) --variant <name>   or: --config <field.json> [--variant <name>]
  --build <dir>             local copy of the build the site serves (calibration, HTML check)
  --runs 6 --settle 3000 --pause 3000 --timeout 30000 --out <dir> --cdp-port <n> --cdp-url <ws>
  Writes to $TMPDIR/perf-kit-replay/<site>-<variant> (owner-only; refuses a git work tree).
  The archive holds REAL responses of the recording account: never copy it anywhere.

replay
  --archive <dir> --dist [name=]<dir> (repeat for an A/B) --runs 6 --calibrate
  --diagnose 1 [--cut gate|lcp|fcp]   coverage + CPU profile run per dist
  --document merged|local|recorded    default merged: the dist's index.html + the tags only the deployed HTML has
  --storage warm|empty      warm: one unmeasured load in the same browser context first, so the build's
                            own storage (cached boot data) is there, as on a returning visit. Default warm
                            when the recorded page had localStorage, else empty
  --bandwidth <Mbit/s> --rtt <ms> --static-ttfb <ms> --wire <kB> --ect slow-2g|2g|3g|4g|off   model overrides
  --out <dir>               default <archive>/replay-report

common
  --json        print one JSON result object on stdout; the log goes to stderr
  --exit-code   replay --calibrate: exit 1 on model-error, 3 on record-again
  exit codes    0 done, 1 model-error (--exit-code), 2 error, 3 record-again (--exit-code)`;

const [command, ...rest] = process.argv.slice(2);
const commands = { record: cmdRecord, inspect: cmdInspect, replay: cmdReplay };
if (!commands[command]) {
  console.log(HELP);
  process.exit(command && command !== 'help' ? 2 : 0);
}
const json = rest.includes('--json');
const exitOnFail = rest.includes('--exit-code');
const argv = rest.filter((a) => a !== '--json' && a !== '--exit-code');
const log = console.log;
if (json) console.log = (...args) => console.error(...args);
try {
  const result = await commands[command](argv);
  if (json) process.stdout.write(`${JSON.stringify(result)}\n`);
  if (exitOnFail && result?.calibration) {
    const codes = { pass: 0, 'model-error': 1, 'record-again': 3 };
    process.exitCode = codes[result.calibration.verdict] ?? 1;
  }
} catch (err) {
  if (json) process.stdout.write(`${JSON.stringify({ error: err.message })}\n`);
  console.error(`error: ${err.message}`);
  process.exitCode = 2;
} finally {
  console.log = log;
}
