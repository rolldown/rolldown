export const RESOURCE_LIMITS = {
  medianWallSpeedupAtLeast: 1.1,
  speedupBootstrapLowerAtLeast: 1.05,
  medianTotalProcessCpuRatioAtMost: 2,
  medianPeakRssRatioAtMost: 2,
  absolutePeakRssBelowBytes: 27 * 1024 ** 3,
};

export function confirmationWorkerCounts(bestCount) {
  if (!Number.isSafeInteger(bestCount) || bestCount < 1 || bestCount > 8) {
    throw new Error('confirmation worker selection requires a best count from one through eight');
  }
  return [...new Set([bestCount - 1, bestCount, bestCount + 1, 4, 8])]
    .filter((count) => count >= 1 && count <= 8)
    .sort((left, right) => left - right);
}

export function createPolicyEvidence(scaleSummaries) {
  return {
    schema: 1,
    jsonPointerBase: '/policyEvidence/byScale',
    byScale: Object.fromEntries(
      scaleSummaries.map((scale) => [
        String(scale.componentCount),
        {
          variants: Object.fromEntries(
            scale.variants.map((variant) => [
              variant.variant,
              {
                wallMedianMs: variant.wallMs.median,
                cpuMedianMs: variant.totalCpuMs.median,
                peakRssMedianBytes: variant.peakRssBytes.median,
                resourceEligible: variant.workerCount === 0 ? true : variant.resourceEligible,
                pairedWallRatioBootstrap95Upper: variant.pairedWallRatioBootstrap95.upper,
                selectedOracleCount: scale.selectedResourceWorkerCount ?? 0,
              },
            ]),
          ),
        },
      ]),
    ),
  };
}

export function classifyMonotonicScreen(screens) {
  if (!Array.isArray(screens) || screens.length < 2) {
    throw new Error('a scale screen requires at least two ordered points');
  }
  for (let index = 0; index < screens.length; index++) {
    const screen = screens[index];
    if (
      !Number.isSafeInteger(screen.componentCount) ||
      !Number.isFinite(screen.speedup) ||
      (index > 0 && screen.componentCount <= screens[index - 1].componentCount)
    ) {
      throw new Error('scale screen points must be finite and strictly increasing');
    }
  }
  const positive = screens.map(({ speedup }) => speedup > 1);
  const firstPositive = positive.indexOf(true);
  const firstReversal =
    firstPositive === -1
      ? -1
      : positive.findIndex((value, index) => index > firstPositive && !value);
  if (firstReversal !== -1) {
    const firstDirectionIndexes =
      firstPositive === 0
        ? [0, Math.min(1, screens.length - 1)]
        : [firstPositive - 1, firstPositive, Math.min(firstPositive + 1, screens.length - 1)];
    return {
      outcome: 'non-monotonic-screen-requires-repeated-direction-evidence',
      selectedScales: [
        ...new Set([
          ...firstDirectionIndexes,
          Math.max(0, firstReversal - 1),
          firstReversal,
          screens.length - 1,
        ]),
      ]
        .sort((left, right) => left - right)
        .map((index) => screens[index].componentCount),
    };
  }
  if (firstPositive === 0) {
    return {
      outcome: 'left-censored-positive-at-smallest-scale',
      selectedScales: [
        screens[0].componentCount,
        screens[1].componentCount,
        screens.at(-1).componentCount,
      ],
    };
  }
  if (firstPositive === -1) {
    return {
      outcome: 'no-positive-base-scale',
      selectedScales: [screens.at(-2).componentCount, screens.at(-1).componentCount],
    };
  }
  return {
    outcome: 'screen-bracketed-first-positive-direction-change',
    selectedScales: [
      screens[firstPositive - 1].componentCount,
      screens[firstPositive].componentCount,
      screens[Math.min(firstPositive + 1, screens.length - 1)].componentCount,
      screens.at(-1).componentCount,
    ],
  };
}

export function selectWorkerWithTieRule(workers) {
  if (!Array.isArray(workers) || workers.length === 0) {
    throw new Error('worker selection requires at least one candidate');
  }
  const fastest = [...workers].sort(
    (left, right) =>
      left.wallMs.median - right.wallMs.median || left.workerCount - right.workerCount,
  )[0];
  const eligible = workers.filter((candidate) => {
    if (candidate.workerCount >= fastest.workerCount) return candidate === fastest;
    const relativeDifference =
      Math.abs(candidate.wallMs.median - fastest.wallMs.median) / fastest.wallMs.median;
    const intervalsOverlap =
      candidate.wallMedianBootstrap95.lower <= fastest.wallMedianBootstrap95.upper &&
      fastest.wallMedianBootstrap95.lower <= candidate.wallMedianBootstrap95.upper;
    return relativeDifference < 0.02 && intervalsOverlap;
  });
  return structuredClone(
    eligible.sort((left, right) => left.workerCount - right.workerCount)[0] ?? fastest,
  );
}

export function isMechanicalGain(worker) {
  return worker.pairedSpeedupBootstrap95.lower > 1;
}

export function isResourceEligible(worker) {
  return (
    worker.pairedSpeedup.median >= RESOURCE_LIMITS.medianWallSpeedupAtLeast &&
    worker.pairedSpeedupBootstrap95.lower >= RESOURCE_LIMITS.speedupBootstrapLowerAtLeast &&
    worker.pairedCpuRatio.median <= RESOURCE_LIMITS.medianTotalProcessCpuRatioAtMost &&
    worker.pairedRssRatio.median <= RESOURCE_LIMITS.medianPeakRssRatioAtMost &&
    worker.peakRssBytes.max < RESOURCE_LIMITS.absolutePeakRssBelowBytes &&
    worker.pagingDeltas.length === 1 &&
    worker.pagingDeltas[0] === JSON.stringify({ pageouts: 0, swapouts: 0 }) &&
    worker.outputCodeHashes.length === 1 &&
    worker.outputMapHashes.length === 1
  );
}

export function resolveConfirmedCrossover(summaries, field, frozenScales) {
  const orderedScales = [...frozenScales].sort((left, right) => left - right);
  const byScale = new Map(summaries.map((summary) => [summary.componentCount, summary]));
  if (
    byScale.size !== summaries.length ||
    summaries.some(({ componentCount }) => !orderedScales.includes(componentCount))
  ) {
    throw new Error(`invalid measured scales while resolving ${field}`);
  }
  const measured = [...summaries].sort((left, right) => left.componentCount - right.componentCount);
  let observedPositive = false;
  for (const summary of measured) {
    if (summary[field]) observedPositive = true;
    else if (observedPositive) {
      return {
        status: 'inconsistent-repeated-direction',
        crossover: null,
        additionalScales: [],
        repeatScales: measured.map(({ componentCount }) => componentCount),
        reason: `repeated ${field} changes from positive back to negative`,
      };
    }
  }

  let candidateIndex = -1;
  for (let index = 0; index < orderedScales.length - 1; index++) {
    const left = byScale.get(orderedScales[index]);
    const right = byScale.get(orderedScales[index + 1]);
    if (left?.[field] && right?.[field]) {
      candidateIndex = index;
      break;
    }
  }
  if (candidateIndex !== -1) {
    while (candidateIndex > 0) {
      const previousScale = orderedScales[candidateIndex - 1];
      const previous = byScale.get(previousScale);
      if (!previous) {
        return {
          status: 'additional-confirmation-required',
          crossover: null,
          additionalScales: [previousScale],
          repeatScales: [],
          reason: `the immediately smaller frozen scale before ${orderedScales[candidateIndex]} was not repeated`,
        };
      }
      if (!previous[field]) {
        return {
          status: 'confirmed',
          crossover: {
            componentCount: orderedScales[candidateIndex],
            confirmedByComponentCount: orderedScales[candidateIndex + 1],
          },
          additionalScales: [],
          repeatScales: [],
        };
      }
      candidateIndex--;
    }
    return {
      status: 'left-censored',
      crossover: {
        atOrBelowComponentCount: orderedScales[0],
        confirmedByComponentCount: orderedScales[1],
      },
      additionalScales: [],
      repeatScales: [],
    };
  }

  const firstPositive = measured.find((summary) => summary[field]);
  if (!firstPositive) {
    if (byScale.get(orderedScales.at(-1))?.[field] === false) {
      return {
        status: 'not-observed-through-maximum',
        crossover: null,
        additionalScales: [],
        repeatScales: [],
      };
    }
    return {
      status: 'additional-confirmation-required',
      crossover: null,
      additionalScales: [orderedScales.at(-1)],
      repeatScales: [],
      reason: 'the maximum frozen scale has not been repeated',
    };
  }

  const index = orderedScales.indexOf(firstPositive.componentCount);
  if (index === orderedScales.length - 1) {
    return {
      status: 'right-boundary-unconfirmed',
      crossover: null,
      additionalScales: [],
      repeatScales: [],
      reason:
        'a gain observed only at the maximum scale has no larger frozen point for confirmation',
    };
  }
  const additionalScales = [];
  if (index > 0 && !byScale.has(orderedScales[index - 1])) {
    additionalScales.push(orderedScales[index - 1]);
  }
  if (!byScale.has(orderedScales[index + 1])) {
    additionalScales.push(orderedScales[index + 1]);
  }
  return {
    status: 'additional-confirmation-required',
    crossover: null,
    additionalScales,
    repeatScales: [],
    reason: `the positive result at ${firstPositive.componentCount} lacks an actual-adjacent frozen confirmation boundary`,
  };
}
