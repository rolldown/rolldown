# Configure Inline Common Chunks

Paste this prompt into your coding agent to let it find the right `maxSize` for [Inline Common Chunks](/in-depth/inline-common-chunks) in your project.

## What you get

The agent adds the option to your build config, with the `maxSize` it picks for your project. In a Vite config, the result looks like this:

```js [vite.config.js]
import { defineConfig } from 'vite';

export default defineConfig({
  build: {
    rolldownOptions: {
      output: {
        codeSplitting: {
          experimentalInlineCommonChunks: {
            maxSize: 24 * 1024, // the value the agent picks
            exclude: /\.(css|scss|sass|less|styl|stylus|pcss|postcss|sss)(\?|$)/,
          },
        },
      },
    },
  },
});
```

The agent also:

- reports how many JS files and gzip bytes each page loads with every value it tried, and why it picked its value;
- leaves a local server running with the new build, so you can try it;
- updates Rolldown or Vite to the latest release of your current major version, if they are behind.

If no value helps your project, the agent leaves the option off and tells you why.

## Prompt

<div class="prompt">

```md
Configure Rolldown's `output.codeSplitting.experimentalInlineCommonChunks` for this project by following the steps below.

The option copies common chunks smaller than `maxSize` into the chunks that use them, so pages load fewer JS files. `maxSize` counts source bytes before minification. How it works and what it costs: https://rolldown.rs/in-depth/inline-common-chunks.md

## Rules

- Change only the build configuration, not application code. The only other change is the Rolldown or Vite update in step 1.
- Ask me before a major upgrade, such as from Vite 7 to Vite 8.
- Write build outputs and measurement files outside the project or to a folder git ignores. Tools such as Tailwind CSS scan the files git does not ignore, and stray files would change the CSS.

## 1. Set up

- Update Rolldown, or Vite in a Vite project, to the latest release of the major version the project uses.
- Build and run the project's tests before changing anything else. If they fail, stop and tell me.
- Apply the option only to the browser build. SSR and library builds keep entry signatures, and Rolldown rejects the option there.
- In a Rolldown config, the option goes in `output.codeSplitting` and needs `output.format: 'es'` and `preserveEntrySignatures: false`.
- In a Vite application, the option goes in `build.rolldownOptions.output.codeSplitting`, and Vite already sets both for the browser build. Add `exclude: /\.(css|scss|sass|less|styl|stylus|pcss|postcss|sss)(\?|$)/` so that CSS is not duplicated across stylesheets.
- If the browser build fails because a framework sets `preserveEntrySignatures` to another value, do not override it. Stop and tell me that the framework does not support the option.

## 2. Choose the loads to measure

- The initial load of each entry.
- The routes users visit most that load through dynamic imports. Opening a route can load several chunks this way, such as a layout and the page; count them all.
- If the router does not show which pages matter most, ask me. If the repository builds several applications, measure each.

## 3. Measure every build from its output

Read the final output chunks, for example in a plugin's `generateBundle` hook with `order: 'post'`, where each chunk has `fileName`, `code`, `imports` and `dynamicImports`. Gzip each JS file separately and record:

- the number of JS files;
- for each initial load, the entry chunk and every chunk it reaches through `imports`, as a file count and gzip bytes;
- for each route, the same for the chunks that open it, counting only files the initial load does not already include;
- for a visit that starts with the initial load and then opens every measured route, the files and gzip bytes of all those loads together, counting each file once;
- the gzip bytes of all JS files together.

## 4. Build the baselines and the range

- The current configuration.
- The strict-execution-order baseline: the current configuration with `output.strictExecutionOrder: true` and no inlining. The option turns strict execution order on, which can change the chunks by itself; this baseline separates that change from what inlining does.
- `maxSize` at 4, 8, 16, 24, 32, 48 and 64 KiB.
- If the configuration has `codeSplitting.groups` or `manualChunks` that only merge small chunks, also build the range without them. Keep groups that serve another purpose, such as a separate vendor chunk for caching.

## 5. Pick the value

- Compare each value with the strict-execution-order baseline: the files it removes from the visit, and the gzip bytes it adds to the visit and to the total.
- Up to some value, raising `maxSize` removes files for few added bytes. At the jump, the added bytes rise sharply while few more files go away, because a large chunk starts being copied into many files.
- Try a value or two between the jump and the value before it.
- Among the values below the jump, or all values if none jumps, pick the smallest one that removes the most files.
- If several applications share the configuration, pick a value below the jump of each.
- If no value removes files, leave the option off.

## 6. Check the application

- Set the chosen configuration and run the project's tests.
- Serve the production build locally, open each measured page, and compare with the current configuration: the pages work the same and no new errors appear in the console.
- If something differs, check the strict-execution-order baseline. When it differs too, strict execution order causes it, not inlining.
- In Vite, the CSS files should be the same as in the strict-execution-order baseline.
- If something breaks only with inlining, adding the module involved to `exclude` may work around it. Tell me, so that I can report it at https://github.com/rolldown/rolldown/issues/11040.
- Keep the server running and give me its URL.

## 7. Compare with the current configuration

The option also turns on strict execution order, so compare the chosen build with the current configuration, using the measurements from step 3 and, if you can drive a browser with network throttling, the medians of several cold loads and of moves between measured pages.

- Recommend the option when the initial loads need fewer files and are not slower.
- Say what it costs when moving between pages, and for returning visitors after a deploy: a changed file makes them download the copies inside it again.

## 8. Report

- Remove anything added only for measuring.
- Give me the versions you updated, a table with one row per build and the measurements from step 3, the chosen configuration and the reason, the comparison with the current configuration, and the check results.
- If another value favors moving between pages, name it as an alternative.
```

</div>
