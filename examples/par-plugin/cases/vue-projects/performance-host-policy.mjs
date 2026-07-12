import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { loadavg } from 'node:os';

const MEBIBYTE = 1024 * 1024;
const TRANSIENT_WAIT_MS = 10_000;
const TRANSIENT_TIMEOUT_MS = 5 * 60_000;

function command(executable, arguments_) {
  const result = spawnSync(executable, arguments_, { encoding: 'utf8' });
  if (result.status !== 0) {
    throw new Error(`${executable} ${arguments_.join(' ')} failed: ${result.stderr}`);
  }
  return result.stdout;
}

function parseByteQuantity(value, unit) {
  const multiplier = { K: 1024, M: MEBIBYTE, G: 1024 ** 3, T: 1024 ** 4 }[unit];
  if (!multiplier) throw new Error(`unsupported byte unit: ${unit}`);
  return Number(value) * multiplier;
}

function swapUsedBytes() {
  const output = command('sysctl', ['-n', 'vm.swapusage']);
  const match = output.match(/used = ([0-9.]+)([KMGT])/);
  if (!match) throw new Error(`could not parse vm.swapusage: ${output.trim()}`);
  return parseByteQuantity(match[1], match[2]);
}

function memoryFreePercentage() {
  const output = command('memory_pressure', ['-Q']);
  const match = output.match(/System-wide memory free percentage:\s*(\d+)%/);
  if (!match) throw new Error(`could not parse memory_pressure -Q: ${output.trim()}`);
  return Number(match[1]);
}

function summedProcessCpuPercentage() {
  return command('ps', ['-A', '-o', '%cpu='])
    .split('\n')
    .filter(Boolean)
    .reduce((total, value) => total + Number(value.trim()), 0);
}

function unrelatedStudyProcesses() {
  const rows = command('ps', ['-A', '-o', 'pid=', '-o', 'ppid=', '-o', 'command='])
    .split('\n')
    .filter(Boolean)
    .map((line) => {
      const match = line.match(/^\s*(\d+)\s+(\d+)\s+(.*)$/);
      return match ? { pid: Number(match[1]), ppid: Number(match[2]), command: match[3] } : null;
    })
    .filter(Boolean);
  const byPid = new Map(rows.map((row) => [row.pid, row]));
  const excluded = new Set([process.pid]);
  let ancestor = process.ppid;
  while (ancestor && !excluded.has(ancestor)) {
    excluded.add(ancestor);
    ancestor = byPid.get(ancestor)?.ppid;
  }
  const studyPattern =
    /(?:rolldown-parallel-js-plugin.*(?:run-(?:performance|matrix)|\b(?:build|test|bench|cargo|rustc|just|vp|pnpm)\b)|(?:^|\s)(?:cargo|rustc|just|vp|pnpm).*(?:build|test|bench).*rolldown)/i;
  return rows
    .filter(({ pid, command: value }) => !excluded.has(pid) && studyPattern.test(value))
    .map(({ pid, command: value }) => ({ pid, commandSha256: sha256(value) }));
}

function sha256(value) {
  return createHash('sha256').update(value).digest('hex');
}

function bootTimeSeconds() {
  const output = command('sysctl', ['-n', 'kern.boottime']);
  const match = output.match(/sec = (\d+)/);
  if (!match) throw new Error(`could not parse kern.boottime: ${output.trim()}`);
  return Number(match[1]);
}

function powerState() {
  const battery = command('pmset', ['-g', 'batt']);
  const settings = command('pmset', ['-g']);
  const thermal = command('pmset', ['-g', 'therm']);
  const lowPowerMatch = settings.match(/^\s*lowpowermode\s+(\d+)$/m);
  if (!lowPowerMatch) throw new Error('could not parse lowpowermode from pmset');
  return {
    acPower: battery.includes("Now drawing from 'AC Power'"),
    lowPowerMode: Number(lowPowerMatch[1]),
    noRecordedThermalWarning: thermal.includes('No thermal warning level has been recorded'),
    noRecordedPerformanceWarning: thermal.includes(
      'No performance warning level has been recorded',
    ),
  };
}

export function virtualMemoryCounters() {
  const output = command('vm_stat', []);
  const read = (label) => {
    const match = output.match(new RegExp(`^${label}:\\s+(\\d+)\\.$`, 'm'));
    if (!match) throw new Error(`could not parse ${label} from vm_stat`);
    return Number(match[1]);
  };
  return { pageouts: read('Pageouts'), swapouts: read('Swapouts') };
}

export function assertNoPagingDelta(before, after) {
  const delta = {
    pageouts: after.pageouts - before.pageouts,
    swapouts: after.swapouts - before.swapouts,
  };
  if (delta.pageouts !== 0 || delta.swapouts !== 0) {
    throw new Error(`formal independent Vue child paged or swapped: ${JSON.stringify(delta)}`);
  }
  return delta;
}

function captureImmediateSnapshot() {
  return {
    ...powerState(),
    uptimeSeconds: Date.now() / 1000 - bootTimeSeconds(),
    swapUsedBytes: swapUsedBytes(),
  };
}

function captureTransientSnapshot() {
  return {
    oneMinuteLoadAverage: loadavg()[0],
    summedProcessCpuPercentage: summedProcessCpuPercentage(),
    memoryFreePercentage: memoryFreePercentage(),
    unrelatedStudyProcesses: unrelatedStudyProcesses(),
  };
}

export function immediateHostFailures(snapshot) {
  const failures = [];
  if (!snapshot.acPower) failures.push('AC power is required');
  if (snapshot.lowPowerMode !== 0) failures.push('low-power mode must be off');
  if (!snapshot.noRecordedThermalWarning) failures.push('thermal warning is recorded');
  if (!snapshot.noRecordedPerformanceWarning) failures.push('performance warning is recorded');
  if (snapshot.uptimeSeconds > 24 * 60 * 60) failures.push('host uptime exceeds 24 hours');
  if (snapshot.swapUsedBytes > 512 * MEBIBYTE) failures.push('swap exceeds 512 MiB');
  return failures;
}

export function transientHostFailures(snapshot) {
  const failures = [];
  if (snapshot.oneMinuteLoadAverage > 2) failures.push('one-minute load average exceeds 2.0');
  if (snapshot.summedProcessCpuPercentage > 150) {
    failures.push('summed process CPU exceeds 150%');
  }
  if (snapshot.memoryFreePercentage < 50) failures.push('free memory percentage is below 50%');
  if ((snapshot.unrelatedStudyProcesses?.length ?? 0) !== 0) {
    failures.push('unrelated study build, test, indexer, or benchmark process is active');
  }
  return failures;
}

function assertDarwin() {
  if (process.platform !== 'darwin') {
    throw new Error('the frozen independent Vue host policy currently supports macOS only');
  }
}

function policy() {
  return {
    maximumUptimeSeconds: 24 * 60 * 60,
    maximumSwapBytes: 512 * MEBIBYTE,
    maximumOneMinuteLoadAverage: 2,
    maximumSummedProcessCpuPercentage: 150,
    minimumMemoryFreePercentage: 50,
    requiredPagingDelta: 0,
  };
}

export async function admitFormalHostBeforeChild() {
  assertDarwin();
  const immediate = captureImmediateSnapshot();
  const immediateFailures = immediateHostFailures(immediate);
  if (immediateFailures.length !== 0) {
    throw new Error(`formal host admission failed: ${immediateFailures.join('; ')}`);
  }
  const waitStartedAt = Date.now();
  let transient;
  while (true) {
    transient = captureTransientSnapshot();
    const failures = transientHostFailures(transient);
    if (failures.length === 0) break;
    if (Date.now() - waitStartedAt >= TRANSIENT_TIMEOUT_MS) {
      throw new Error(`transient host admission timed out: ${failures.join('; ')}`);
    }
    await new Promise((resolve) => setTimeout(resolve, TRANSIENT_WAIT_MS));
  }
  return {
    phase: 'before-child',
    admittedAt: new Date().toISOString(),
    ...immediate,
    ...transient,
    waitedMs: Date.now() - waitStartedAt,
    policy: policy(),
  };
}

export function admitFormalHostAfterChild() {
  assertDarwin();
  const snapshot = { ...captureImmediateSnapshot(), ...captureTransientSnapshot() };
  const failures = [...immediateHostFailures(snapshot), ...transientHostFailures(snapshot)];
  if (failures.length !== 0) {
    throw new Error(`post-child formal host admission failed: ${failures.join('; ')}`);
  }
  return {
    phase: 'after-child',
    admittedAt: new Date().toISOString(),
    ...snapshot,
    waitedMs: 0,
    policy: policy(),
  };
}
