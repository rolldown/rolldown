---
name: upgrade-oxc
description: 'Upgrade oxc, run codegen, and fix any breaking changes.'
---

# Upgrade OXC

CRITICAL: Run each step sequentially ONE AT A TIME. Wait for each command to FULLY COMPLETE before proceeding to the next step. DO NOT run multiple commands in parallel - they have dependencies on each other.

## Steps

1. `git checkout main && git pull origin main`
2. `just setup`
3. `npm view @oxc-project/types version` - note the version
4. Edit `pnpm-workspace.yaml`: update `@oxc-project/runtime`, `@oxc-project/types`, `oxc-minify`, `oxc-parser`, `oxc-transform` to the version from step 3 (use `=x.y.z` format)
5. `cargo search oxc_allocator --limit 1` - note the version
6. `cargo search oxc_resolver --limit 1` - note the version
7. Edit `Cargo.toml`: update `oxc`, `oxc_allocator`, `oxc_ecmascript`, `oxc_minify_napi`, `oxc_parser_napi`, `oxc_transform_napi`, `oxc_traverse` to version from step 5; update `oxc_resolver`, `oxc_resolver_napi` to version from step 6
8. `cargo update oxc oxc_allocator oxc_ecmascript oxc_minify_napi oxc_parser_napi oxc_transform_napi oxc_traverse oxc_resolver oxc_resolver_napi oxc_sourcemap oxc_index`
9. `pnpm install` - install updated npm packages
10. `cargo check` - if there are errors, fix all breaking changes before proceeding. Common breaking changes include renamed types, changed method signatures, or removed APIs. Study the error messages carefully and update the code accordingly.
11. `just update-generated-code`
12. `just test-update`
13. `just ued`
14. `just roll` - run the full build, lint, and test suite. Fix any remaining breaking changes or test failures before proceeding.
15. `git status --short && git diff --stat` - verify expected files changed
16. Summarize the upgrade by reporting: (a) the old and new versions, (b) number of files changed, (c) any breaking changes that were fixed, and (d) notable changes in the diff
