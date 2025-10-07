# Why Plugin Hook Filters?

## The Problem

Even though Rolldown's core is written in Rust with parallel processing capabilities, **adding JavaScript plugins can significantly slow down your builds**. Why? Because each plugin hook gets called for _every_ module, even when the plugin doesn't care about most of them.

For example, if you have a CSS plugin that only transforms `.css` files, it still gets called for every `.js`, `.ts`, `.jsx`, and other file in your project. With 10 plugins, this overhead multiplies, causing build times to increase by **3-4x**.

Plugin hook filters solve this by letting Rolldown skip unnecessary plugin calls at the Rust level, keeping your builds fast even with many plugins.

## Real-World Impact

Let's see the actual performance difference with a benchmark using [apps/10000](https://github.com/rolldown/benchmarks/tree/main/apps/10000):
branch: https://github.com/rolldown/benchmarks/pull/3

```diff
diff --git a/apps/10000/rolldown.config.mjs b/apps/10000/rolldown.config.mjs
--- a/apps/10000/rolldown.config.mjs
+++ b/apps/10000/rolldown.config.mjs
@@ -1,8 +1,25 @@
 import { defineConfig } from "rolldown";
-import { minify } from "rollup-plugin-esbuild";
+// import { minify } from "rollup-plugin-esbuild";
 const sourceMap = !!process.env.SOURCE_MAP;
 const m = !!process.env.MINIFY;
+const transformPluginCount = process.env.PLUGIN_COUNT || 0;
 
+let transformCssPlugin = Array.from({ length: transformPluginCount }, (_, i) => {
+  let index = i + 1;
+  return {
+    name: `transform-css-${index}`,
+    transform(code, id) {
+      if (id.endsWith(`foo${index}.css`)) {
+        return {
+          code: `.index-${index} {
+  color: red;
+}`,
+          map: null,
+        };
+      }
+    }
+  }
+})
 export default defineConfig({
 	input: {
 		main: "./src/index.jsx",
@@ -11,13 +28,7 @@ export default defineConfig({
 		"process.env.NODE_ENV": JSON.stringify("production"),
 	},
 	plugins: [
-		m
-			? minify({
-					minify: true,
-					legalComments: "none",
-					target: "es2022",
-				})
-			: null,
+    ...transformCssPlugin,
 	].filter(Boolean),
 	profilerNames: !m,
 	output: {
diff --git a/apps/10000/src/index.css b/apps/10000/src/index.css
deleted file mode 100644
diff --git a/apps/10000/src/index.jsx b/apps/10000/src/index.jsx
--- a/apps/10000/src/index.jsx
+++ b/apps/10000/src/index.jsx
@@ -1,7 +1,16 @@
 import React from "react";
 import ReactDom from "react-dom/client";
 import App1 from "./f0";
-import './index.css'
+import './foo1.css'
+import './foo2.css'
+import './foo3.css'
+import './foo4.css'
+import './foo5.css'
+import './foo6.css'
+import './foo7.css'
+import './foo8.css'
+import './foo9.css'
+import './foo10.css'
 
 ReactDom.createRoot(document.getElementById("root")).render(
 	<React.StrictMode>
```

**Setup:**

- 10 CSS files (`foo1.css` to `foo10.css`)
- Each plugin transforms only one specific CSS file (e.g., plugin 1 only cares about `foo1.css`)
- Variable number of plugins controlled via `PLUGIN_COUNT`
- Plugins use standard pattern: check if file matches, return early if not

### Without Filter (Traditional Approach)

```bash
Benchmark 1: PLUGIN_COUNT=0 node --run build:rolldown
  Time (mean ± σ):     745.6 ms ±  11.8 ms    [User: 2298.0 ms, System: 1161.3 ms]
  Range (min … max):   732.1 ms … 753.6 ms    3 runs
 
Benchmark 2: PLUGIN_COUNT=1 node --run build:rolldown
  Time (mean ± σ):     862.6 ms ±  61.3 ms    [User: 2714.1 ms, System: 1192.6 ms]
  Range (min … max):   808.3 ms … 929.2 ms    3 runs
 
Benchmark 3: PLUGIN_COUNT=2 node --run build:rolldown
  Time (mean ± σ):      1.106 s ±  0.020 s    [User: 3.287 s, System: 1.382 s]
  Range (min … max):    1.091 s …  1.130 s    3 runs
 
Benchmark 4: PLUGIN_COUNT=5 node --run build:rolldown
  Time (mean ± σ):      1.848 s ±  0.022 s    [User: 4.398 s, System: 1.728 s]
  Range (min … max):    1.825 s …  1.869 s    3 runs
 
Benchmark 5: PLUGIN_COUNT=10 node --run build:rolldown
  Time (mean ± σ):      2.792 s ±  0.065 s    [User: 6.013 s, System: 2.198 s]
  Range (min … max):    2.722 s … 2.850 s    3 runs
 
Summary
 'PLUGIN_COUNT=0 node --run build:rolldown' ran
    1.16 ± 0.08 times faster than 'PLUGIN_COUNT=1 node --run build:rolldown'
    1.48 ± 0.04 times faster than 'PLUGIN_COUNT=2 node --run build:rolldown'
    2.48 ± 0.05 times faster than 'PLUGIN_COUNT=5 node --run build:rolldown'
    3.74 ± 0.10 times faster than 'PLUGIN_COUNT=10 node --run build:rolldown'
```

**Key Takeaway:** Build time scales linearly with plugin count - 10 plugins = **3.74x slower** (2.8s vs 745ms).

## The Solution: Plugin Hook Filters

Instead of calling every plugin for every module, use `filter` to tell Rolldown which files each plugin cares about. Here's how:

```diff
diff --git a/apps/10000/rolldown.config.mjs b/apps/10000/rolldown.config.mjs
index 822af995..dee07e68 100644
--- a/apps/10000/rolldown.config.mjs
+++ b/apps/10000/rolldown.config.mjs
@@ -8,14 +8,21 @@ let transformCssPlugin = Array.from({ length: transformPluginCount }, (_, i) =>
   let index = i + 1;
   return {
     name: `transform-css-${index}`,
-    transform(code, id) {
-      if (id.endsWith(`foo${index}.css`)) {
-        return {
-          code: `.index-${index} {
+    transform: {
+      filter: {
+        id: {
+          include: new RegExp(`foo${index}.css$`),
+        }
+      },
+      handler(code, id) {
+        if (id.endsWith(`foo${index}.css`)) {
+          return {
+            code: `.index-${index} {
   color: red;
 }`,
-          map: null,
-        };
+            map: null,
+          };
+        }
       }
     }
   }
```

**What changed:**

- Wrapped the `transform` function in an object with `handler` and `filter` properties
- Added `filter.id.include` with a regex pattern matching only the files this plugin cares about
- Rolldown now checks the filter in Rust _before_ calling into JavaScript

### With Filter (Optimized)

```bash
Benchmark 1: PLUGIN_COUNT=0 node --run build:rolldown
  Time (mean ± σ):     739.1 ms ±   6.8 ms    [User: 2312.5 ms, System: 1153.0 ms]
  Range (min … max):   733.0 ms … 746.5 ms    3 runs
 
Benchmark 2: PLUGIN_COUNT=1 node --run build:rolldown
  Time (mean ± σ):     760.6 ms ±  18.3 ms    [User: 2422.1 ms, System: 1107.4 ms]
  Range (min … max):   739.7 ms … 773.6 ms    3 runs
 
Benchmark 3: PLUGIN_COUNT=2 node --run build:rolldown
  Time (mean ± σ):     731.2 ms ±  11.1 ms    [User: 2461.3 ms, System: 1141.4 ms]
  Range (min … max):   723.9 ms … 744.0 ms    3 runs
 
Benchmark 4: PLUGIN_COUNT=5 node --run build:rolldown
  Time (mean ± σ):     741.5 ms ±   9.3 ms    [User: 2621.6 ms, System: 1111.3 ms]
  Range (min … max):   734.0 ms … 751.9 ms    3 runs
 
Benchmark 5: PLUGIN_COUNT=10 node --run build:rolldown
  Time (mean ± σ):     747.3 ms ±   2.1 ms    [User: 2900.9 ms, System: 1120.0 ms]
  Range (min … max):   745.0 ms … 749.2 ms    3 runs
 
Summary
  'PLUGIN_COUNT=2 node --run build:rolldown' ran
    1.01 ± 0.02 times faster than 'PLUGIN_COUNT=0 node --run build:rolldown'
    1.01 ± 0.02 times faster than 'PLUGIN_COUNT=5 node --run build:rolldown'
    1.02 ± 0.02 times faster than 'PLUGIN_COUNT=10 node --run build:rolldown'
    1.04 ± 0.03 times faster than 'PLUGIN_COUNT=1 node --run build:rolldown'
```

**Key Takeaway:** With filters, all plugin counts perform nearly identically (~740ms). The overhead has been **eliminated**.

### Performance Comparison

| Plugin Count | Without Filter | With Filter | Speedup   |
| ------------ | -------------- | ----------- | --------- |
| 0 plugins    | 745ms          | 739ms       | 1.0x      |
| 1 plugin     | 863ms          | 761ms       | 1.13x     |
| 2 plugins    | 1,106ms        | 731ms       | 1.51x     |
| 5 plugins    | 1,848ms        | 742ms       | 2.49x     |
| 10 plugins   | 2,792ms        | 747ms       | **3.74x** |

**Bottom line:** When you have plugins that only care about specific files, use filters to maintain fast build times regardless of how many plugins you add.

## How It Works Under the Hood

To understand why filters are so effective, you need to understand how Rolldown processes modules with JavaScript plugins.

Rolldown uses parallel processing (like the [producer-consumer problem](https://en.wikipedia.org/wiki/Producer%E2%80%93consumer_problem)) to build the module graph efficiently. Here's a simple dependency graph to illustrate:

**Dependency Graph**
![dependency graph](https://github.com/user-attachments/assets/e49c29f1-1d2f-4d21-a277-311bcc33eda7)

### Without JavaScript Plugins

![Bundling without JavaScript plugins](https://github.com/user-attachments/assets/ad071cf9-6a34-4a7d-a669-02efec342d45)

Everything runs in parallel in Rust. Multiple CPU cores process modules simultaneously, maximizing throughput.

> [!NOTE]
> These diagrams show the conceptual algorithm, not exact implementation details. Some time slices are exaggerated for clarity—`fetch_module` actually runs at macrosecond speeds.

### With JavaScript Plugins (No Filter)

![Bundling with JavaScript plugins](https://github.com/user-attachments/assets/7e95fb60-d345-4d23-a35e-c7d062fa2b70)

Here's the bottleneck: **JavaScript plugins run in a single thread**. Even though Rolldown's Rust core is parallel, every module must:

1. Stop at the "diamond" (hook call phase)
2. Cross the FFI boundary from Rust → JavaScript
3. Wait for _all_ plugins to run serially
4. Cross back from JavaScript → Rust

This serialization point becomes a major bottleneck. Notice how the diamond section grows wider as more plugins are added, while CPU cores sit idle waiting for JavaScript.

### With Filters (Optimized)

When you add filters, Rolldown evaluates them **in Rust** before crossing into JavaScript:

```
For each module:
  For each plugin:
    ✓ Check filter in Rust (macrosecond)
    ✗ Skip if no match
    → Only call JavaScript for matching plugins
```

This eliminates the majority of FFI overhead and JavaScript execution time. In the benchmark, most plugins don't match most files, so nearly all calls are skipped. The diamond shrinks back down, CPU utilization stays high, and build times remain fast.

## When to Use Filters

**Use filters when:**

- ✅ Your plugin only processes specific file types (e.g., `.css`, `.svg`, `.md`)
- ✅ Your plugin targets specific directories (e.g., `src/**`, `node_modules/**`)
- ✅ You have multiple plugins in your build
- ✅ You care about build performance

## Quick Reference

```js
// ❌ Without filter - called for every module
export default {
  name: 'my-plugin',
  transform(code, id) {
    if (!id.endsWith('.css')) return;
    // ... transform CSS
  },
};

// ✅ With filter - only called for CSS files
export default {
  name: 'my-plugin',
  transform: {
    filter: {
      id: { include: /\.css$/ },
    },
    handler(code, id) {
      // ... transform CSS
    },
  },
};
```

See the [plugin hook filter usage](/plugins/hook-filters) for complete filter api and options.
