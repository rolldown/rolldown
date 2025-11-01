# Resolver Integration Tests

This directory contains integration tests for oxc-resolver functionality used by Rolldown.

## Test Cases

### 1. `oxc_resolver_integration`
Tests comprehensive resolver scenarios including:
- **Conditional exports**: Verifies that `import`/`require` conditions are respected
- **Subpath exports**: Tests package exports with subpath patterns
- **Nested conditions**: Validates complex conditional export resolution with platform-specific conditions

### 2. `package_exports_wildcard`
Tests wildcard patterns in package.json exports field:
- Validates that `./features/*` and `./utils/*` patterns resolve correctly
- Ensures multiple wildcard exports can coexist in the same package

### 3. `self_referencing_package`
Tests self-referencing packages using package name in exports:
- Validates that a package can import from itself using its package name
- Tests internal module resolution via package exports

### 4. `extensionless_typescript`
Tests TypeScript extension resolution:
- Validates that `.js` imports resolve to `.ts` files
- Validates that `.mjs` imports resolve to `.mts` files
- Implements esbuild's extension rewriting behavior

### 5. `browser_field_resolution`
Tests browser field resolution when platform is set to browser:
- Validates that the `browser` field in package.json is respected
- Ensures platform-specific entry points are correctly resolved

## Purpose

These tests validate oxc-resolver integration and are designed to:
1. Demonstrate expected resolver behavior for various edge cases
2. Provide regression tests for resolver improvements
3. Validate compatibility with Node.js module resolution algorithm
4. Test package.json exports field handling

## Running Tests

```bash
# Run all resolver tests
just test-rust -- resolve

# Run specific test
cargo test --package rolldown --test integration_rolldown fixture_with_config_tests__rolldown__resolve__oxc_resolver_integration
```

## Test Structure

Each test follows the standard Rolldown test pattern:
- `_config.json`: Test configuration
- `main.js`: Entry point with assertions
- `node_modules/`: Mock npm packages with various configurations
- `artifacts.snap`: Expected output snapshot (auto-generated)

## Notes

- These tests currently pass with the existing oxc-resolver version
- They serve as integration tests for resolver functionality
- If you're implementing new resolver features, you may need to update these tests to mark expected behavior

## Related

- oxc-project/oxc-resolver: The resolver library used by Rolldown
- Rolldown resolver integration: `crates/rolldown_resolver/`
