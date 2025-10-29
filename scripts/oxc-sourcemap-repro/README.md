# Oxc Codegen Sourcemap Bug Reproduction

This is a minimal reproduction script demonstrating the bug in oxc_codegen where it generates sourcemap tokens with invalid source positions (beyond the source content bounds).

## The Bug

When `oxc_codegen` generates JavaScript code with sourcemaps enabled:
1. It adds punctuation (semicolons, newlines) that don't exist in the original source
2. It creates sourcemap tokens for these added characters
3. These tokens reference positions beyond the end of source lines (e.g., column 22 when source only has 22 characters, ending at column 21)
4. oxc_sourcemap v6's stricter validation detects these as `[invalid]` tokens

## Running the Reproduction

```bash
cd scripts/oxc-sourcemap-repro
cargo run
```

## Expected Output

You should see output showing multiple test cases where oxc_codegen generates invalid sourcemap tokens:

```
Test 1: Export statement without trailing semicolon
-----------------------------------------------
Source: "export default { foo }"
Length: 22 characters (valid columns: 0-21)

Generated: "export default { foo };\n"

Sourcemap Analysis:
  ❌ Found 1 INVALID sourcemap token(s)!

  Invalid tokens:
    (0:22) [invalid] --> (0:22)
```

The `(0:22)` source position is invalid because the source only has 22 characters (columns 0-21).

## Impact on Rolldown

This bug affects rolldown when:
1. Rolldown parses source code with oxc
2. Transforms the AST (renaming, rewriting, etc.)
3. Calls oxc_codegen to generate code with sourcemaps
4. The generated sourcemaps contain invalid tokens

The workaround in rolldown is to filter out these invalid tokens after generation.

## Related Issues

- Rolldown issue: https://github.com/rolldown/rolldown/issues/6054
- Rolldown PR: https://github.com/rolldown/rolldown/pull/[PR_NUMBER]

## Version Info

- oxc: 0.95.0
- oxc_sourcemap: 6.0.0
