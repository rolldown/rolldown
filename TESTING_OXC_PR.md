# Testing oxc PR #15069 - ✅ CONFIRMED FIXED!

## Status

✅ **CONFIRMED**: The oxc PR #15069 successfully fixes the invalid sourcemap token generation issue!

## oxc PR Details

- PR: https://github.com/oxc-project/oxc/pull/15069
- Commit: https://github.com/oxc-project/oxc/commit/3fbb307367a31817a297dee299fec580912675db
- Fix: Prevents oxc_codegen from creating sourcemap tokens for positions beyond source content

## Test Results

Added `[patch.crates-io]` section to Cargo.toml using the specific commit hash and ran full test suite:

### Before (with invalid tokens)
```
(2:22) [invalid] --> (10:32)
(7:42) [invalid] --> (16:42)
```

### After (fixed!)
```
(2:17) "foo }" --> (10:20) "foo: foo$1 };\n"
```

The added semicolons and newlines are now properly included in the previous token's range instead of creating invalid tokens.

### Test Results
- ✅ All 601 tests pass
- ✅ Zero `[invalid]` markers in all test snapshots
- ✅ Sourcemaps are now valid and properly map generated code

## Implementation

Added patch section to Cargo.toml:
```toml
[patch.crates-io]
oxc = { git = "https://github.com/oxc-project/oxc", rev = "3fbb307367a31817a297dee299fec580912675db" }
# ... (all oxc crates patched)
```

## Next Steps

1. ✅ Confirmed the fix works
2. Wait for oxc v0.96.0 (or next version) to be released with this fix
3. Update rolldown's oxc dependency to the new version
4. Remove the `[patch.crates-io]` section
5. Close the issue - no workaround needed in rolldown!
