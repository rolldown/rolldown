/**
 * Mock implementation of the Platform enum from Rust
 */
export const Platform = {
  Node: 0,
  Browser: 1,
  Neutral: 2,
  Wasi: 3,
  WasiP2: 4,
};

/**
 * Try to convert a string to a Platform enum value
 */
export function tryFrom(value) {
  switch (value) {
    case "node": return Platform.Node;
    case "browser": return Platform.Browser;
    case "neutral": return Platform.Neutral;
    case "wasi":
    case "wasip1": return Platform.Wasi;
    case "wasip2": return Platform.WasiP2;
    default: throw new Error(`Unknown platform: ${value}`);
  }
} 