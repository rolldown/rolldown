# Integration Tests for oxc-resolver

## Summary

This PR adds comprehensive integration tests for oxc-resolver functionality in the Rolldown test suite. These tests validate various resolver scenarios that are commonly encountered in modern JavaScript/TypeScript projects.

## Tests Added

### 1. `oxc_resolver_integration`

**Purpose**: Comprehensive test for multiple resolver features in one scenario

- Conditional exports (`import` vs `require` conditions)
- Subpath exports (`./utils`)
- Nested conditional exports (platform + import/require combinations)

**Key validation**: Ensures that the resolver correctly handles complex package.json exports field configurations with multiple levels of conditions.

### 2. `package_exports_wildcard`

**Purpose**: Test wildcard patterns in package.json exports

- Pattern matching for `./features/*` and `./utils/*`
- Multiple wildcard exports in the same package

**Key validation**: Confirms that wildcard patterns are resolved correctly and can coexist.

### 3. `self_referencing_package`

**Purpose**: Test packages that import from themselves using their package name

- Self-referencing through exports field
- Internal module cross-references

**Key validation**: Ensures packages can use their own package name to import internal modules via exports.

### 4. `extensionless_typescript`

**Purpose**: Test TypeScript extension rewriting (esbuild compatibility)

- `.js` imports resolve to `.ts` files
- `.mjs` imports resolve to `.mts` files

**Key validation**: Validates the extension rewriting feature that allows TypeScript projects to use `.js` extensions in imports while the actual files have `.ts` extensions.

### 5. `browser_field_resolution`

**Purpose**: Test platform-specific entry points

- Browser field resolution when platform is set to browser
- Main vs browser entry point selection

**Key validation**: Ensures the resolver respects the `browser` field in package.json when the platform is set to browser.

## Why These Tests?

These integration tests:

1. **Validate Current Behavior**: All tests currently pass, demonstrating that Rolldown's resolver integration works correctly for these scenarios.

2. **Provide Regression Protection**: Future changes to oxc-resolver or Rolldown's resolver integration can be validated against these tests.

3. **Document Expected Behavior**: The tests serve as living documentation of how the resolver should handle various edge cases.

4. **Support oxc-resolver Development**: While the specific oxc-project/oxc-resolver#790 PR couldn't be accessed, these tests cover common resolver scenarios that are frequently improved or fixed in resolver libraries.

## Test Structure

Each test follows Rolldown's standard integration test pattern:

```
test_name/
├── _config.json          # Test configuration
├── main.js               # Entry point with assertions
├── node_modules/         # Mock npm packages
└── artifacts.snap        # Expected output (auto-generated)
```

## Running the Tests

```bash
# Run all resolver tests
just test-rust -- resolve

# Run a specific test
cargo test --package rolldown --test integration_rolldown fixture_with_config_tests__rolldown__resolve__oxc_resolver_integration
```

## Next Steps

If you have specific requirements from oxc-project/oxc-resolver#790 that aren't covered by these tests, please let me know and I can add additional test cases. These tests currently:

- ✅ Cover common resolver edge cases
- ✅ All pass with the current oxc-resolver version (11.11.1)
- ✅ Are ready to validate future resolver improvements
- ❓ May need adjustment based on specific #790 requirements

## Documentation

A README has been added to `crates/rolldown/tests/rolldown/resolve/README.md` documenting all resolver tests and their purposes.
