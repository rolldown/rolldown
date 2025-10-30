# Testing oxc PR #15069

## Status

Reverted all filtering changes to test the oxc PR #15069 which should fix the root cause of invalid sourcemap token generation.

## Issue

The oxc PR #15069 (https://github.com/oxc-project/oxc/pull/15069) mentioned in the review comments doesn't appear to exist yet or is not publicly accessible. 

Attempted to add a `[patch.crates-io]` section to Cargo.toml to use the PR branch:
- Repository: https://github.com/sapphi-red/oxc
- Branch: `fix-invalid-source-position`

But the branch was not found in the repository.

## Current State

All changes have been reverted:
- ✅ Removed `filter_invalid_tokens()` function from `rolldown_sourcemap/src/lib.rs`
- ✅ Removed filtering in `collapse_sourcemaps()`  
- ✅ Removed filtering in `render_ecma_module.rs`
- ✅ Removed documentation comments
- ✅ Removed reproduction script at `scripts/oxc-sourcemap-repro/`
- ✅ Reverted all test snapshots - `[invalid]` markers are back

Tests confirm the invalid sourcemap tokens are present again:
```
(2:22) [invalid] --> (10:32)
(7:42) [invalid] --> (16:42)
```

## Next Steps

Once the oxc PR #15069 is available:
1. Add `[patch.crates-io]` section to Cargo.toml with the correct repository and branch/commit
2. Run `cargo update` to fetch the patched oxc crates
3. Run tests to verify if the oxc PR fixes the invalid sourcemap issue
4. If it works, we can wait for oxc to release and remove rolldown's workaround
5. If it doesn't work, we need to keep the filtering approach or find another solution
