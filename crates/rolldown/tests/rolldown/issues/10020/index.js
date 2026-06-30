// Default-import a JSON module and read keys dynamically.
// Mirrors vinext's packages/vinext/src/build/google-fonts/fallback-metrics.ts:
//   import rawFallbackMetrics from "./fallback-metrics-data.json" with { type: "json" };
import metrics from "./data.json" with { type: "json" };

export function get(name) {
  return metrics[name];
}
