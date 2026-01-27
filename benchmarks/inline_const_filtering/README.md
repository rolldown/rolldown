# Inline Const Filtering Optimization Benchmark

This benchmark measures the performance impact of the module filtering optimization
in the cross-module inline const optimization pass.

## Background

When `optimization.inlineConst.pass` is set to > 1, rolldown performs multiple iterations
of cross-module constant propagation. Without filtering, each iteration would process
all modules. With the optimization, subsequent iterations only process modules that
import newly discovered constants.

## Test Case

- 15-level constant chain: `CONST_0 → CONST_1 → ... → CONST_14`
- 10,000 unrelated modules (each with 13 exported constants)
- 1 main entry importing `CONST_14`
- Total: 10,016 modules

## Running the Benchmark

```bash
# Generate test files (creates src/ directory with 10,016 modules)
node generate.mjs

# Run benchmark
node bench.mjs
```

## Expected Results

With the filtering optimization:

- Iteration 1: All 10,016 modules processed (full iteration)
- Iterations 2-14: Only ~15 modules processed each (filtered iterations)

The key metric is the **cost per additional filtered iteration**, which should be
very low (< 5ms) since filtered iterations skip 10,000+ unrelated modules.

## Sample Output

```
| Pass | Time (ms) | Extra iterations | Δ from pass=1 |
|------|-----------|------------------|---------------|
|    1 |     479.4 |                0 |          +0.0 |
|    2 |     476.6 |                1 |          -2.7 |
|    5 |     481.4 |                4 |          +2.1 |
|   10 |     481.6 |                9 |          +2.3 |
|   15 |     498.4 |               14 |         +19.0 |

Incremental cost from pass=2 to pass=15: 21.7ms
Average cost per additional filtered iteration: 1.67ms
```

## Interpretation

- The cross-module optimization phase is fast relative to total bundling time (~480ms)
- Adding 13 filtered iterations (pass=2 to pass=15) only adds ~22ms total
- This demonstrates the filtering is working: each filtered iteration processes
  only ~15 modules instead of 10,016 modules

Without the filtering optimization, each additional iteration would need to
process all 10,016 modules, making pass=15 significantly slower.
