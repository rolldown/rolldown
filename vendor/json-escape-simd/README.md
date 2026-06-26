# json-escape-simd

![Crates.io Version](https://img.shields.io/crates/v/json-escape-simd)
![docs.rs](https://img.shields.io/docsrs/json-escape-simd)
[![CodSpeed Badge](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://codspeed.io/napi-rs/json-escape-simd)

Optimized SIMD routines for escaping JSON strings. The implementation is from [sonic-rs](https://github.com/cloudwego/sonic-rs), we only take the string escaping part to avoid the abstraction overhead.

## Benchmarks

Numbers below come from `cargo bench` runs on GitHub Actions hardware. Criterion reports are summarized to make it easier to spot relative performance. "vs fastest" shows how much slower each implementation is compared to the fastest entry in the table (1.00× means fastest).

### GitHub Actions x86_64 (`ubuntu-latest`)

`AVX2` enabled.

**Short string payload (~89M iterations)**

| Implementation        | Median time  | vs fastest |
| --------------------- | ------------ | ---------- |
| **`escape simd`**     | **56.23 ns** | **1.00×**  |
| `escape generic`      | 111.58 ns    | 1.98×      |
| `serde_json`          | 111.65 ns    | 1.99×      |
| `json-escape`         | 140.01 ns    | 2.49×      |
| `escape v_jsonescape` | 151.98 ns    | 2.70×      |
| `escape sonic`        | 202.21 ns    | 3.60×      |

**RxJS payload (~10k iterations)**

| Implementation        | Median time   | vs fastest |
| --------------------- | ------------- | ---------- |
| **`escape simd`**     | **246.84 µs** | **1.00×**  |
| `escape sonic`        | 261.87 µs     | 1.06×      |
| `escape v_jsonescape` | 531.77 µs     | 2.15×      |
| `json-escape`         | 561.93 µs     | 2.28×      |
| `escape generic`      | 664.00 µs     | 2.69×      |
| `serde_json`          | 649.19 µs     | 2.63×      |

**Fixtures payload (~300 iterations)**

| Implementation        | Median time   | vs fastest |
| --------------------- | ------------- | ---------- |
| **`escape simd`**     | **9.1461 ms** | **1.00×**  |
| `escape sonic`        | 9.3410 ms     | 1.02×      |
| `json-escape`         | 19.077 ms     | 2.09×      |
| `escape v_jsonescape` | 20.015 ms     | 2.19×      |
| `serde_json`          | 21.240 ms     | 2.32×      |
| `escape generic`      | 22.512 ms     | 2.46×      |

### GitHub Actions aarch64 (`ubuntu-24.04-arm`)

Neon enabled.

**Short string payload (~76M iterations)**

| Implementation        | Median time  | vs fastest |
| --------------------- | ------------ | ---------- |
| **`escape simd`**     | **65.42 ns** | **1.00×**  |
| `escape generic`      | 108.28 ns    | 1.66×      |
| `serde_json`          | 110.73 ns    | 1.69×      |
| `json-escape`         | 150.64 ns    | 2.30×      |
| `escape v_jsonescape` | 183.41 ns    | 2.80×      |
| `escape sonic`        | 212.87 ns    | 3.25×      |

**RxJS payload (~10k iterations)**

| Implementation        | Median time   | vs fastest |
| --------------------- | ------------- | ---------- |
| **`escape simd`**     | **283.40 µs** | **1.00×**  |
| `escape sonic`        | 305.34 µs     | 1.08×      |
| `json-escape`         | 468.04 µs     | 1.65×      |
| `escape generic`      | 548.50 µs     | 1.94×      |
| `serde_json`          | 567.23 µs     | 2.00×      |
| `escape v_jsonescape` | 758.18 µs     | 2.68×      |

**Fixtures payload (~300 iterations)**

| Implementation        | Median time  | vs fastest |
| --------------------- | ------------- | ---------- |
| **`escape simd`**     | **10.46 ms** | **1.00×**  |
| `escape sonic`        | 11.18 ms     | 1.07×      |
| `json-escape`         | 15.45 ms     | 1.48×      |
| `escape generic`      | 17.75 ms     | 1.70×      |
| `serde_json`          | 18.01 ms     | 1.72×      |
| `escape v_jsonescape` | 24.93 ms     | 2.38×      |

### GitHub Actions macOS (`macos-latest`)

> Apple M1 chip

**Short string payload (~45M iterations)**

| Implementation        | Median time   | vs fastest |
| --------------------- | ------------- | ---------- |
| **`escape simd`**     | **113.16 ns** | **1.00×**  |
| `serde_json`          | 128.52 ns     | 1.14×      |
| `escape generic`      | 136.06 ns     | 1.20×      |
| `json-escape`         | 173.16 ns     | 1.53×      |
| `escape v_jsonescape` | 198.80 ns     | 1.76×      |
| `escape sonic`        | 226.21 ns     | 2.00×      |

**RxJS payload (~10k iterations)**

| Implementation        | Median time   | vs fastest |
| --------------------- | ------------- | ---------- |
| **`escape sonic`**    | **379.94 µs** | **1.00×**  |
| `escape simd`         | 395.41 µs     | 1.04×      |
| `json-escape`         | 618.74 µs     | 1.63×      |
| `escape generic`      | 690.86 µs     | 1.82×      |
| `serde_json`          | 720.05 µs     | 1.89×      |
| `escape v_jsonescape` | 839.86 µs     | 2.21×      |

**AFFiNE sources payload (~300 iterations)**

| Implementation        | Median time  | vs fastest |
| --------------------- | ------------ | ---------- |
| **`escape simd`**     | **12.60 ms** | **1.00×**  |
| `escape sonic`        | 14.04 ms     | 1.11×      |
| `json-escape`         | 21.54 ms     | 1.71×      |
| `serde_json`          | 23.33 ms     | 1.85×      |
| `escape generic`      | 24.55 ms     | 1.95×      |
| `escape v_jsonescape` | 26.82 ms     | 2.13×      |

### Apple M3 Max

**Short string benchmark**

| Implementation        | Median time   | vs fastest |
| --------------------- | ------------- | ---------- |
| **`escape simd`**     | **90.58 ns**  | **1.00×**  |
| `serde_json`          | 139.23 ns     | 1.54×      |
| `escape generic`      | 146.15 ns     | 1.61×      |
| `json-escape`         | 173.60 ns     | 1.92×      |
| `escape v_jsonescape` | 198.60 ns     | 2.19×      |
| `escape sonic`        | 199.27 ns     | 2.20×      |

**RxJS payload (~10k iterations)**

| Implementation        | Median time   | vs fastest |
| --------------------- | ------------- | ---------- |
| **`escape simd`**     | **196.07 µs** | **1.00×**  |
| `escape sonic`        | 196.32 µs     | 1.00×      |
| `json-escape`         | 446.94 µs     | 2.28×      |
| `escape generic`      | 488.37 µs     | 2.49×      |
| `serde_json`          | 553.08 µs     | 2.82×      |
| `escape v_jsonescape` | 618.31 µs     | 3.15×      |

**AFFiNE sources payload (~300 iterations)**

| Implementation        | Median time  | vs fastest |
| --------------------- | ------------ | ---------- |
| **`escape simd`**     | **10.36 ms** | **1.00×**  |
| `escape sonic`        | 10.57 ms     | 1.02×      |
| `escape generic`      | 17.61 ms     | 1.70×      |
| `json-escape`         | 18.01 ms     | 1.74×      |
| `serde_json`          | 19.00 ms     | 1.83×      |
| `escape v_jsonescape` | 21.38 ms     | 2.06×      |
