import { heavyPrepaintTypes, weightLabel } from './measure.mjs';

/** A gain in one paint metric must not hide a regression in the other. */
export function comparePaints(deltas, { noiseFloorMs, noiseFloorPct }) {
  const metrics = Object.fromEntries(
    ['fcp', 'lcp'].map((paint) => {
      const delta = deltas?.[`runtime.${paint}_ms`];
      if (!delta || !Number.isFinite(delta.prev) || !Number.isFinite(delta.delta)) {
        return [paint, { state: 'unknown' }];
      }
      const thresholdMs = Math.max(noiseFloorMs, (noiseFloorPct / 100) * delta.prev);
      const state =
        delta.delta <= -thresholdMs
          ? 'improved'
          : delta.delta >= thresholdMs
            ? 'regressed'
            : 'within-noise';
      return [paint, { ...delta, thresholdMs, state }];
    }),
  );
  const states = Object.values(metrics).map((metric) => metric.state);
  const state = states.includes('regressed')
    ? 'regressed'
    : states.includes('unknown')
      ? 'unknown'
      : states.includes('improved')
        ? 'improved'
        : 'within-noise';
  return { state, metrics };
}

/** Concurrent observations are candidates to inspect, not proof that paint waited. */
export function renderGapEvidence(report) {
  const evidence = [];
  for (const paint of ['fcp', 'lcp']) {
    const scripts = report.postLoadScripts?.[paint];
    if (scripts?.count > 0) {
      evidence.push({
        kind: 'script',
        paint,
        detail: `${scripts.count} script(s) started after load and finished before ${paint.toUpperCase()} (${(scripts.bytes / 1024).toFixed(1)}KiB)`,
        next: `Inspect chunkDiscovery.${paint} and the request initiator. Check which downloads rendering awaits before changing preload timing.`,
      });
    }
  }
  const fetches = report.gatingFetches ?? [];
  if (fetches.length) {
    evidence.push({
      kind: 'fetch',
      paint: 'fcp',
      detail: `fetch/XHR before FCP: ${fetches.join('; ')}`,
      next: 'Check whether initial rendering awaits these responses. Test moving only optional data reads after paint.',
    });
  }
  for (const row of heavyPrepaintTypes(report.resourceWeight)) {
    evidence.push({
      kind: row.type,
      paint: 'fcp',
      detail: `before FCP: ${weightLabel(row)}`,
      next:
        row.type === 'font'
          ? 'Check which fonts visible text needs. Test a smaller font subset or fewer font variants before paint.'
          : 'Check which image is the LCP element. Keep its request early. Test deferring images outside the first view.',
    });
  }
  return evidence;
}

export function chunkDiscoveryAdvice(paint) {
  const target = paint === 'fcp' ? 'the first visible content' : 'the LCP element';
  return (
    `Trace the import chain and request initiator in the source. Check whether the chunk renders ${target}. ` +
    'If paint needs it, test an earlier preload or a different chunk boundary. ' +
    'If paint does not need it, test deferring its load and work. Compare both FCP and LCP with the pinned baseline.'
  );
}
