import { spawnSync } from 'node:child_process';
import { loadavg } from 'node:os';

const MEBIBYTE = 1024 * 1024;
const TRANSIENT_WAIT_MS = 10_000;
const TRANSIENT_TIMEOUT_MS = 5 * 60_000;

const command = (executable, arguments_) => {
  const result = spawnSync(executable, arguments_, { encoding: 'utf8' });
  if (result.status !== 0) {
    throw new Error(`${executable} ${arguments_.join(' ')} failed: ${result.stderr}`);
  }
  return result.stdout;
};

const parseByteQuantity = (value, unit) => {
  const multiplier = { K: 1024, M: MEBIBYTE, G: 1024 ** 3, T: 1024 ** 4 }[unit];
  if (!multiplier) throw new Error(`unsupported byte unit: ${unit}`);
  return Number(value) * multiplier;
};

const swapUsedBytes = () => {
  const output = command('sysctl', ['-n', 'vm.swapusage']);
  const match = output.match(/used = ([0-9.]+)([KMGT])/);
  if (!match) throw new Error(`could not parse vm.swapusage: ${output.trim()}`);
  return parseByteQuantity(match[1], match[2]);
};

const memoryFreePercentage = () => {
  const output = command('memory_pressure', ['-Q']);
  const match = output.match(/System-wide memory free percentage:\s*(\d+)%/);
  if (!match) throw new Error(`could not parse memory_pressure -Q: ${output.trim()}`);
  return Number(match[1]);
};

const summedProcessCpuPercentage = () => {
  const output = command('ps', ['-A', '-o', '%cpu=']);
  return output
    .split('\n')
    .filter(Boolean)
    .reduce((total, value) => total + Number(value.trim()), 0);
};

const bootTimeSeconds = () => {
  const output = command('sysctl', ['-n', 'kern.boottime']);
  const match = output.match(/sec = (\d+)/);
  if (!match) throw new Error(`could not parse kern.boottime: ${output.trim()}`);
  return Number(match[1]);
};

const powerState = () => {
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
};

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
    throw new Error(`formal Vue scale child paged or swapped: ${JSON.stringify(delta)}`);
  }
  return delta;
}

export const captureImmediateHostSnapshot = () => {
  const power = powerState();
  return {
    ...power,
    uptimeSeconds: Date.now() / 1000 - bootTimeSeconds(),
    swapUsedBytes: swapUsedBytes(),
  };
};

export const assertImmediateHostGates = (snapshot) => {
  const failures = [];
  if (!snapshot.acPower) failures.push('AC power is required');
  if (snapshot.lowPowerMode !== 0) failures.push('low-power mode must be off');
  if (!snapshot.noRecordedThermalWarning) failures.push('thermal warning is recorded');
  if (!snapshot.noRecordedPerformanceWarning) failures.push('performance warning is recorded');
  if (snapshot.uptimeSeconds > 24 * 60 * 60) failures.push('host uptime exceeds 24 hours');
  if (snapshot.swapUsedBytes > 512 * MEBIBYTE) failures.push('starting swap exceeds 512 MiB');
  if (failures.length !== 0)
    throw new Error(`formal host admission failed: ${failures.join('; ')}`);
};

const transientSnapshot = () => ({
  oneMinuteLoadAverage: loadavg()[0],
  summedProcessCpuPercentage: summedProcessCpuPercentage(),
  memoryFreePercentage: memoryFreePercentage(),
});

const transientFailures = (snapshot) => {
  const failures = [];
  if (snapshot.oneMinuteLoadAverage > 2) failures.push('one-minute load average exceeds 2.0');
  if (snapshot.summedProcessCpuPercentage > 150) {
    failures.push('summed pre-child process CPU exceeds 150%');
  }
  if (snapshot.memoryFreePercentage < 50) failures.push('free memory percentage is below 50%');
  return failures;
};

export async function admitFormalHost() {
  if (process.platform !== 'darwin') {
    throw new Error('the frozen Vue scale host policy currently supports macOS only');
  }
  const immediate = captureImmediateHostSnapshot();
  assertImmediateHostGates(immediate);
  const waitStartedAt = Date.now();
  let transient;
  while (true) {
    transient = transientSnapshot();
    const failures = transientFailures(transient);
    if (failures.length === 0) break;
    if (Date.now() - waitStartedAt >= TRANSIENT_TIMEOUT_MS) {
      throw new Error(`transient host admission timed out: ${failures.join('; ')}`);
    }
    await new Promise((resolve) => setTimeout(resolve, TRANSIENT_WAIT_MS));
  }
  return {
    admittedAt: new Date().toISOString(),
    ...immediate,
    ...transient,
    waitedMs: Date.now() - waitStartedAt,
    policy: {
      maximumUptimeSeconds: 24 * 60 * 60,
      maximumStartingSwapBytes: 512 * MEBIBYTE,
      maximumOneMinuteLoadAverage: 2,
      maximumSummedProcessCpuPercentage: 150,
      minimumMemoryFreePercentage: 50,
      requiredPagingDelta: 0,
    },
  };
}

export function admitFormalHostAfterChild() {
  if (process.platform !== 'darwin') {
    throw new Error('the frozen Vue scale host policy currently supports macOS only');
  }
  const immediate = captureImmediateHostSnapshot();
  assertImmediateHostGates(immediate);
  return {
    admittedAt: new Date().toISOString(),
    ...immediate,
    policy: {
      requiredAcPower: true,
      requiredLowPowerMode: 0,
      requiredNoRecordedThermalWarning: true,
      requiredNoRecordedPerformanceWarning: true,
      maximumUptimeSeconds: 24 * 60 * 60,
      maximumSwapUsedBytes: 512 * MEBIBYTE,
    },
  };
}
