# Release Rolldown

## Publish Latest

1. Visit https://github.com/rolldown/rolldown/actions/workflows/prepare-release.yml
2. "Run workflow" with `1.0.0-beta.x` (without leading `v`).
3. Wait for https://github.com/rolldown/rolldown/actions/workflows/publish-to-npm.yml to finish.

## Canary

1. Visit https://github.com/rolldown/rolldown/actions/workflows/publish-to-npm-for-nightly-canary.yml
2. "Run workflow"

Latest Canary versions are https://www.npmjs.com/package/rolldown/v/canary

## pkg.pr.new

https://pkg.pr.new/~/rolldown/rolldown

# Standard Operating Procedure for Debugging Rolldown Issues

This section provides a systematic approach to debugging issues in Rolldown. Follow these procedures to efficiently identify, isolate, and resolve problems.

## Initial Triage and Information Gathering

### 1. Gather Basic Information

Before diving into debugging, collect essential information:

- **Rolldown version**: Check the version being used
- **Operating system**: Windows, macOS, Linux, and specific versions
- **Node.js version**: Ensure compatibility with project requirements
- **Error messages**: Complete error output with stack traces
- **Configuration**: Rolldown config file and relevant options
- **Project structure**: Size and complexity of the project being bundled
- **Recent changes**: What changed before the issue appeared

### 2. Classify the Issue Type

Categorize the issue to determine the appropriate debugging approach:

- **Build failures**: Compilation errors, syntax issues, dependency problems
- **Runtime errors**: Issues in the bundled output when executed
- **Performance issues**: Slow build times, high memory usage
- **Incorrect output**: Wrong bundling behavior, missing/extra files
- **Plugin issues**: Problems with specific plugins or plugin interactions
- **API compatibility**: Differences from expected Rollup behavior

## Reproduction and Isolation

### 3. Create a Minimal Reproduction Case

Follow these steps to isolate the issue:

1. **Start with the failing project**:
   ```bash
   # Backup the current state
   git stash
   # or copy the project to a safe location
   ```

2. **Reduce the project systematically**:
   - Remove non-essential files and dependencies
   - Simplify the configuration
   - Remove plugins one by one
   - Minimize the input files

3. **Create a standalone reproduction**:
   ```bash
   # Create a new minimal project
   mkdir rolldown-issue-reproduction
   cd rolldown-issue-reproduction
   npm init -y
   npm install rolldown
   # Add minimal files that reproduce the issue
   ```

4. **Verify the reproduction**:
   - Ensure the issue still occurs with minimal setup
   - Test with different Rolldown versions if relevant
   - Document the exact steps to reproduce

### 4. Environment Testing

Test across different environments to isolate environment-specific issues:

- **Different operating systems**: Especially if Windows-specific
- **Different Node.js versions**: Check against supported versions
- **Different package managers**: npm, pnpm, yarn
- **Clean installations**: Fresh node_modules and lock files

## Using Built-in Debugging Tools

### 5. Enable Comprehensive Logging

Rolldown provides powerful logging capabilities. Use them systematically:

1. **Start with basic debug logging**:
   ```bash
   RD_LOG=debug rolldown [your-command]
   ```

2. **Use module-specific logging** for targeted debugging:
   ```bash
   # For resolver issues
   RD_LOG='oxc_resolver' rolldown [your-command]

   # For specific modules
   RD_LOG='rolldown_core=debug' rolldown [your-command]

   # Multiple modules
   RD_LOG='rolldown_core=debug,rolldown_binding=trace' rolldown [your-command]
   ```

3. **Generate trace files** for performance analysis:
   ```bash
   RD_LOG=debug RD_LOG_OUTPUT=chrome-json rolldown [your-command]
   # Open the generated trace file in Chrome DevTools
   ```

4. **Function-level tracing** for specific areas:
   ```bash
   RD_LOG='[function_name]' rolldown [your-command]
   ```

### 6. Memory Profiling

For memory-related issues, use heaptrack (Linux/WSL only):

1. **Build with memory profiling support**:
   ```bash
   just build-memory-profile
   ```

2. **Run with heaptrack**:
   ```bash
   heaptrack node ./path/to/your/script.js
   # or with version managers:
   heaptrack $(asdf which node) ./path/to/your/script.js
   ```

3. **Analyze the results** in the heaptrack GUI that opens automatically

### 7. Performance Profiling

For performance issues:

1. **Use built-in timing information** with debug logs
2. **Profile with Chrome DevTools** using trace output
3. **Compare with known benchmarks** from the project's benchmark suite
4. **Test with release builds** for accurate performance measurements:
   ```bash
   just build native release
   ```

## Creating Test Cases

### 8. Write Targeted Tests

Create tests to verify fixes and prevent regressions:

1. **For Rust-side issues**, create tests in `/crates/rolldown/tests/`:
   ```bash
   # Create a new test directory
   mkdir crates/rolldown/tests/fixtures/your-issue-name
   cd crates/rolldown/tests/fixtures/your-issue-name

   # Create test files
   echo '{"input": ["main.js"]}' > _config.json
   echo 'console.log("test");' > main.js
   ```

2. **For Node.js API issues**, create tests in `/packages/rolldown/tests/`:
   ```javascript
   // In packages/rolldown/tests/fixtures/your-test/
   // Create test files and rolldown.config.js
   ```

3. **Run the specific test**:
   ```bash
   # For Rust tests
   just test-rust

   # For Node.js tests
   just test-node rolldown -t your-test-name
   ```

4. **Update snapshots** when adding new tests:
   ```bash
   just test-update
   ```

## Common Debugging Scenarios

### 9. Build Failures

1. **Check for syntax errors** in input files
2. **Verify dependency resolution**:
   ```bash
   RD_LOG='oxc_resolver=debug' rolldown [your-command]
   ```
3. **Test with minimal configuration**
4. **Check for plugin conflicts** by removing plugins one by one
5. **Verify file permissions** and paths

### 10. Runtime Errors

1. **Compare bundled output** with expected results
2. **Check for module resolution issues**
3. **Verify export/import mappings**
4. **Test the bundled output** in isolation:
   ```bash
   node dist/your-bundle.js
   ```
5. **Check source maps** for debugging bundled code

### 11. Performance Issues

1. **Profile memory usage** with heaptrack
2. **Analyze build timing** with trace output
3. **Check for plugin performance** impact
4. **Test on different operating systems** (Windows can be significantly slower)
5. **Consider using Dev Drive on Windows** or WSL for better performance
6. **Review plugin hook filters** to reduce overhead

### 12. Plugin Issues

1. **Test without the problematic plugin**
2. **Check plugin compatibility** with current Rolldown version
3. **Verify plugin configuration**
4. **Test with hook filters** to optimize performance:
   ```javascript
   import { withFilter } from 'rolldown/filter';

   export default {
     plugins: [
       withFilter(yourPlugin(), { transform: { id: /\.ext$/ } })
     ]
   };
   ```

## Advanced Debugging Techniques

### 13. Source Code Investigation

When logs and profiling aren't sufficient:

1. **Review recent commits** related to the failing area
2. **Add temporary debug prints** to Rust code:
   ```rust
   tracing::debug!("Debug info: {:?}", variable);
   ```
3. **Build and test with your changes**:
   ```bash
   just build native debug
   ```

### 14. Bisection for Regressions

If an issue appeared recently:

1. **Use git bisect** to find the problematic commit:
   ```bash
   git bisect start
   git bisect bad  # current broken state
   git bisect good [last-known-good-commit]
   # Follow git bisect prompts, testing each commit
   ```

2. **Test each commit** with your reproduction case
3. **Identify the specific change** that caused the regression

## Issue Escalation and Reporting

### 15. When to Escalate

Escalate to the core team when:

- The issue appears to be a fundamental design problem
- The debugging process reveals potential security issues
- Performance regressions affect major use cases
- The issue blocks critical functionality
- Multiple users report similar problems

### 16. Preparing Issue Reports

When reporting issues, include:

1. **Minimal reproduction case** with clear setup instructions
2. **Complete environment information** (OS, Node.js, Rolldown versions)
3. **Full error messages and logs** with relevant `RD_LOG` output
4. **Steps attempted** during debugging
5. **Expected vs. actual behavior**
6. **Potential workarounds** discovered
7. **Impact assessment** (how many users affected, severity)

### 17. Following Up

After reporting:

1. **Monitor issue responses** and provide additional information if requested
2. **Test proposed fixes** with your reproduction case
3. **Verify fixes** across different environments
4. **Update issue status** when resolved or if workarounds are found

## Maintenance and Prevention

### 18. Keeping Debug Tools Updated

Regularly maintain debugging capabilities:

1. **Update trace logging** in new code areas
2. **Add tests for resolved issues** to prevent regressions
3. **Document new debugging techniques** discovered
4. **Keep benchmark suites** updated for performance monitoring

### 19. Knowledge Sharing

Share debugging knowledge:

1. **Document common solutions** in this SOP
2. **Update troubleshooting guides** with new findings
3. **Share insights** in team discussions and issue comments
4. **Contribute to testing infrastructure** improvements

---

For more detailed information on specific debugging tools and techniques, refer to:
- [Tracing/Logging Guide](docs/contrib-guide/tracing-logging.md)
- [Profiling Guide](docs/contrib-guide/profiling.md)
- [Testing Guide](docs/contrib-guide/testing.md)
- [Troubleshooting Guide](docs/guide/troubleshooting.md)
