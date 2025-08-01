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

5. **Common logging patterns for specific issues**:
   ```bash
   # Module resolution debugging
   RD_LOG='oxc_resolver=trace' rolldown build

   # Bundle generation issues
   RD_LOG='rolldown_core::chunk_graph=debug' rolldown build

   # Plugin execution tracing
   RD_LOG='rolldown_plugin=trace' rolldown build

   # AST transformation debugging
   RD_LOG='rolldown_ecmascript=debug' rolldown build
   ```

6. **Interpreting log output**:
   - **TRACE level**: Very detailed execution flow, function entry/exit
   - **DEBUG level**: General debugging information, state changes
   - **INFO level**: Important events and milestones
   - **WARN level**: Potential issues that don't stop execution
   - **ERROR level**: Actual errors that may cause failures

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

**Creating comprehensive test cases:**

1. **For Rust-side issues**, create tests in `/crates/rolldown/tests/`:
   ```bash
   # Create a new test directory
   mkdir crates/rolldown/tests/fixtures/your-issue-name
   cd crates/rolldown/tests/fixtures/your-issue-name

   # Create test configuration
   cat > _config.json << 'EOF'
   {
     "input": ["main.js"],
     "external": ["external-dep"]
   }
   EOF

   # Create test files
   cat > main.js << 'EOF'
   import { helper } from './helper.js';
   import external from 'external-dep';

   console.log(helper(), external);
   EOF

   cat > helper.js << 'EOF'
   export function helper() {
     return 'Hello from helper';
   }
   EOF

   # Expected output (optional)
   cat > _expected.js << 'EOF'
   // Expected bundled output for comparison
   EOF
   ```

2. **For Node.js API issues**, create tests in `/packages/rolldown/tests/`:
   ```javascript
   // packages/rolldown/tests/fixtures/your-test/rolldown.config.js
   export default {
     input: './main.js',
     plugins: [
       // Test-specific plugins
       {
         name: 'test-plugin',
         transform(code, id) {
           if (id.endsWith('.special')) {
             return { code: `export default ${JSON.stringify(code)}` };
           }
         }
       }
     ]
   };

   // main.js
   import special from './file.special';
   console.log(special);

   // file.special
   This is special content that should be transformed
   ```

3. **Complex scenario testing**:
   ```javascript
   // Test circular dependencies
   // packages/rolldown/tests/fixtures/circular-deps/

   // a.js
   import { b } from './b.js';
   export const a = 'a' + b;

   // b.js
   import { a } from './a.js';
   export const b = 'b' + (a || '');

   // main.js
   import { a } from './a.js';
   import { b } from './b.js';
   console.log(a, b);
   ```

4. **Running and updating tests**:
   ```bash
   # Run specific Rust test
   cargo test --package rolldown -- --nocapture your_issue_name

   # Run with detailed output
   RUST_LOG=debug cargo test --package rolldown -- --nocapture your_issue_name

   # Run Node.js tests
   cd packages/rolldown
   npm test -- --testNamePattern="your-test-name"

   # Update snapshots for Rust tests
   cargo insta review

   # Update snapshots for Node.js tests
   npm test -- --updateSnapshot
   ```

5. **Performance regression tests**:
   ```javascript
   // Create benchmark test
   // packages/rolldown/tests/fixtures/performance/large-project/

   // generate-large-project.js
   const fs = require('fs');
   const path = require('path');

   // Generate many files to test performance
   for (let i = 0; i < 1000; i++) {
     const content = `
       import dep${(i + 1) % 100} from './dep${(i + 1) % 100}.js';
       export const module${i} = 'module-${i}-' + dep${(i + 1) % 100};
     `;
     fs.writeFileSync(`module${i}.js`, content);
   }

   // Then measure build time and memory usage
   ```

## Common Debugging Scenarios

### 9. Build Failures

**Common patterns and solutions:**

1. **Syntax errors in input files**:
   ```bash
   # Enable parser debugging
   RD_LOG='oxc_parser=debug' rolldown build
   # Look for parsing errors in the output
   ```

2. **Dependency resolution issues**:
   ```bash
   # Debug module resolution
   RD_LOG='oxc_resolver=debug' rolldown build

   # Common resolution problems:
   # - Missing package.json
   # - Incorrect import paths
   # - Node.js vs browser resolution conflicts
   # - TypeScript path mapping issues
   ```

3. **Plugin configuration errors**:
   ```bash
   # Test without plugins
   rolldown build --config rolldown.config.minimal.js

   # Add plugins back one by one
   # Check plugin hook execution order
   RD_LOG='rolldown_plugin=trace' rolldown build
   ```

4. **Memory or resource exhaustion**:
   ```bash
   # Monitor memory usage during build
   RD_LOG=debug rolldown build 2>&1 | grep -i memory

   # Check for circular dependencies
   RD_LOG='rolldown_core::chunk_graph=debug' rolldown build
   ```

5. **File permission issues**:
   ```bash
   # Check file permissions
   ls -la input/files/

   # Verify output directory permissions
   ls -la output/directory/
   ```

### 10. Runtime Errors

**Systematic approach to runtime debugging:**

1. **Compare bundled output** with expected results:
   ```bash
   # Generate source maps for easier debugging
   rolldown build --sourcemap

   # Examine the bundled output structure
   cat dist/your-bundle.js | head -50
   ```

2. **Module resolution and import/export issues**:
   ```javascript
   // Common runtime errors:
   // - "Cannot read property of undefined"
   // - "Module is not defined"
   // - "Unexpected token 'export'"

   // Debug by checking module boundaries in output
   grep -n "// [module-path]" dist/your-bundle.js
   ```

3. **Verify export/import mappings**:
   ```bash
   # Check how modules are bundled together
   RD_LOG='rolldown_core::chunk_graph=debug' rolldown build

   # Look for export/import transformation issues
   RD_LOG='rolldown_ecmascript=debug' rolldown build
   ```

4. **Test the bundled output** in isolation:
   ```bash
   # Test in Node.js
   node dist/your-bundle.js

   # Test in browser (create minimal HTML)
   echo '<script src="dist/your-bundle.js"></script>' > test.html
   ```

5. **Debug with source maps**:
   ```javascript
   // Enable source map support in Node.js
   node --enable-source-maps dist/your-bundle.js

   // In browser DevTools, ensure source maps are loaded
   // Check "Sources" tab for original file structure
   ```

6. **Common runtime error patterns**:
   ```bash
   # ESM/CJS compatibility issues
   RD_LOG='rolldown_core::module_loader=debug' rolldown build

   # Circular dependency runtime issues
   RD_LOG='rolldown_core::chunk_graph=trace' rolldown build

   # Hoisting and temporal dead zone issues
   RD_LOG='rolldown_ecmascript::ast=debug' rolldown build
   ```

### 11. Performance Issues

1. **Profile memory usage** with heaptrack
2. **Analyze build timing** with trace output
3. **Check for plugin performance** impact
4. **Test on different operating systems** (Windows can be significantly slower)
5. **Consider using Dev Drive on Windows** or WSL for better performance
6. **Review plugin hook filters** to reduce overhead

### 12. Plugin Issues

**Comprehensive plugin debugging workflow:**

1. **Isolate the problematic plugin**:
   ```javascript
   // Create a minimal config without the plugin
   export default {
     input: 'src/main.js',
     plugins: [] // Remove all plugins first
   };

   // Add plugins back one by one to identify the culprit
   ```

2. **Plugin compatibility and configuration**:
   ```bash
   # Check plugin version compatibility
   npm list rolldown-plugin-*

   # Verify plugin configuration syntax
   RD_LOG='rolldown_plugin=debug' rolldown build
   ```

3. **Plugin hook execution debugging**:
   ```javascript
   // Add debug logging to custom plugins
   export default function myPlugin() {
     return {
       name: 'my-plugin',
       buildStart() {
         console.log('[my-plugin] buildStart called');
       },
       resolveId(id, importer) {
         console.log(`[my-plugin] resolveId: ${id} from ${importer}`);
         return null; // Let other plugins handle it
       },
       transform(code, id) {
         console.log(`[my-plugin] transform: ${id}`);
         return { code, map: null };
       }
     };
   }
   ```

4. **Performance-optimized plugin usage** with hook filters:
   ```javascript
   import { withFilter } from 'rolldown/filter';

   export default {
     plugins: [
       // Only apply to specific file types
       withFilter(myPlugin(), {
         transform: { id: /\.(js|ts)$/ },
         resolveId: { id: /^@my-scope/ }
       }),

       // More specific filtering examples
       withFilter(cssPlugin(), {
         transform: { id: /\.css$/ }
       }),

       withFilter(jsonPlugin(), {
         resolveId: { id: /\.json$/ },
         load: { id: /\.json$/ }
       })
     ]
   };
   ```

5. **Plugin hook order and conflicts**:
   ```bash
   # Trace plugin execution order
   RD_LOG='rolldown_plugin=trace' rolldown build 2>&1 | grep -E "(buildStart|resolveId|load|transform)"

   # Look for plugin conflicts in specific hooks
   RD_LOG='rolldown_plugin::hook_runner=debug' rolldown build
   ```

6. **Common plugin issues and solutions**:
   ```javascript
   // Issue: Plugin modifies shared state
   // Solution: Ensure plugin state is isolated per build
   function problematicPlugin() {
     let sharedState = {}; // ❌ Shared across builds

     return {
       name: 'problematic',
       buildStart() {
         sharedState = {}; // ✅ Reset per build
       }
     };
   }

   // Issue: Plugin returns incorrect hook result format
   // Solution: Follow Rolldown plugin API exactly
   function correctPlugin() {
     return {
       name: 'correct',
       transform(code, id) {
         // ✅ Always return object with code property
         return { code: modifiedCode, map: sourceMap };
         // ❌ Don't return just the code string
       }
     };
   }
   ```

## Advanced Debugging Techniques

### 13. IDE Integration and Development Workflows

**Setting up debugging in popular IDEs:**

1. **Visual Studio Code debugging setup**:
   ```json
   // .vscode/launch.json
   {
     "version": "0.2.0",
     "configurations": [
       {
         "name": "Debug Rolldown Build",
         "type": "node",
         "request": "launch",
         "program": "${workspaceFolder}/node_modules/.bin/rolldown",
         "args": ["build", "--config", "rolldown.config.js"],
         "env": {
           "RD_LOG": "debug"
         },
         "console": "integratedTerminal",
         "cwd": "${workspaceFolder}"
       },
       {
         "name": "Debug Rolldown with Custom Script",
         "type": "node",
         "request": "launch",
         "program": "${workspaceFolder}/debug-script.js",
         "env": {
           "RD_LOG": "rolldown_core=debug"
         },
         "console": "integratedTerminal"
       }
     ]
   }
   ```

2. **Creating debugging scripts**:
   ```javascript
   // debug-script.js - Custom debugging script
   import { rolldown } from 'rolldown';

   async function debugBuild() {
     console.log('Starting debug build...');

     try {
       const bundle = await rolldown({
         input: 'src/main.js',
         plugins: [
           // Add your plugins here
         ]
       });

       const { output } = await bundle.generate({
         format: 'es'
       });

       console.log('Generated chunks:', output.length);
       output.forEach(chunk => {
         console.log(`- ${chunk.fileName}: ${chunk.code.length} chars`);
       });

     } catch (error) {
       console.error('Build failed:', error);
       console.error('Stack trace:', error.stack);
     }
   }

   debugBuild();
   ```

3. **Debugging with watch mode**:
   ```javascript
   // watch-debug.js - Debug with file watching
   import { watch } from 'rolldown';

   const watcher = watch({
     input: 'src/main.js',
     output: {
       dir: 'dist',
       format: 'es'
     },
     plugins: [
       // Debug plugin to trace changes
       {
         name: 'debug-watcher',
         buildStart() {
           console.log(`[${new Date().toISOString()}] Build started`);
         },
         buildEnd() {
           console.log(`[${new Date().toISOString()}] Build completed`);
         },
         watchChange(id) {
           console.log(`[${new Date().toISOString()}] File changed: ${id}`);
         }
       }
     ]
   });

   watcher.on('event', event => {
     console.log('Watcher event:', event.code, event.result?.output?.length || 0, 'chunks');
   });
   ```

### 14. Source Code Investigation

**Deep diving into Rolldown's codebase:**

1. **Understanding the codebase structure**:
   ```bash
   # Explore the main crates
   find crates -name "*.rs" | head -20

   # Key areas for debugging:
   # - crates/rolldown_core/ - Main bundling logic
   # - crates/rolldown_plugin/ - Plugin system
   # - crates/rolldown_resolver/ - Module resolution
   # - crates/rolldown_ecmascript/ - JS/TS processing
   ```

2. **Adding debug prints to Rust code**:
   ```rust
   // In Rust source files, add tracing statements
   use tracing::{debug, trace, info, warn, error};

   // Example debugging in bundling logic
   pub fn process_module(&self, module: &Module) -> Result<()> {
       debug!("Processing module: {:?}", module.id);
       trace!("Module content preview: {:?}", &module.code[..100.min(module.code.len())]);

       // Your existing logic here

       debug!("Finished processing module: {:?}", module.id);
       Ok(())
   }
   ```

3. **Building and testing with modifications**:
   ```bash
   # Build debug version with your changes
   cargo build

   # Or build release for performance testing
   cargo build --release

   # Test specific functionality
   cargo test --package rolldown -- --nocapture test_name
   ```

4. **Using Rust debugging tools**:
   ```bash
   # Run with Rust backtrace
   RUST_BACKTRACE=1 RD_LOG=debug rolldown build

   # Full backtrace for complex issues
   RUST_BACKTRACE=full RD_LOG=trace rolldown build

   # Use GDB for native debugging (Linux/macOS)
   gdb --args node ./node_modules/.bin/rolldown build
   ```

5. **Identifying relevant code sections**:
   ```bash
   # Search for specific functionality
   rg "resolve.*module" crates/ --type rust

   # Find error message sources
   rg "Failed to.*bundle" crates/ --type rust

   # Locate plugin hook implementations
   rg "fn.*transform" crates/ --type rust
   ```

### 15. Environment-Specific Debugging

**Debugging across different environments:**

1. **Windows-specific issues**:
   ```bash
   # Use Windows-specific paths and tools
   # PowerShell debugging
   $env:RD_LOG="debug"; rolldown build

   # Check for path separator issues
   RD_LOG='oxc_resolver=debug' rolldown build 2>&1 | findstr /C:"path"

   # Use WSL for better performance if available
   wsl -e bash -c "cd /mnt/c/your/project && RD_LOG=debug rolldown build"
   ```

2. **macOS debugging**:
   ```bash
   # Use instruments for performance profiling
   xcrun xctrace record --template "Time Profiler" --launch -- node ./node_modules/.bin/rolldown build

   # Debug with lldb
   lldb -- node ./node_modules/.bin/rolldown build
   ```

3. **Linux containers and CI environments**:
   ```bash
   # Debug in Docker
   docker run -v $(pwd):/app -w /app node:18 bash -c "npm install && RD_LOG=debug npm run build"

   # CI-specific debugging
   # Add debug output to CI scripts
   export RD_LOG=debug
   export RUST_BACKTRACE=1
   rolldown build 2>&1 | tee build.log
   ```

4. **Memory-constrained environments**:
   ```bash
   # Monitor memory usage during build
   # Linux
   /usr/bin/time -v rolldown build

   # macOS
   /usr/bin/time -l rolldown build

   # Reduce memory usage with smaller chunk sizes
   RD_LOG='rolldown_core::chunk_graph=debug' rolldown build --max-chunk-size=1mb
   ```

### 16. Bisection for Regressions

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

### 17. When to Escalate

Escalate to the core team when:

- The issue appears to be a fundamental design problem
- The debugging process reveals potential security issues
- Performance regressions affect major use cases
- The issue blocks critical functionality
- Multiple users report similar problems

### 18. Preparing Issue Reports

When reporting issues, include:

1. **Minimal reproduction case** with clear setup instructions
2. **Complete environment information** (OS, Node.js, Rolldown versions)
3. **Full error messages and logs** with relevant `RD_LOG` output
4. **Steps attempted** during debugging
5. **Expected vs. actual behavior**
6. **Potential workarounds** discovered
7. **Impact assessment** (how many users affected, severity)

### 19. Following Up

After reporting:

1. **Monitor issue responses** and provide additional information if requested
2. **Test proposed fixes** with your reproduction case
3. **Verify fixes** across different environments
4. **Update issue status** when resolved or if workarounds are found

## Debugging Specific Error Patterns

### 20. Common Error Messages and Solutions

**Error pattern recognition and systematic resolution:**

1. **"Cannot resolve module" errors**:
   ```bash
   # Enable resolver debugging
   RD_LOG='oxc_resolver=trace' rolldown build

   # Common causes and solutions:
   # - Missing file extension in import
   # - Incorrect relative path
   # - Missing package in node_modules
   # - TypeScript path mapping issues

   # Debug resolution paths
   node -e "console.log(require.resolve.paths('your-module'))"
   ```

2. **"Unexpected token" errors**:
   ```bash
   # Check for syntax issues in specific files
   RD_LOG='oxc_parser=debug' rolldown build

   # Common causes:
   # - Mixed ESM/CommonJS syntax
   # - Unsupported JavaScript features
   # - Incorrect file encoding
   # - Plugin transformation issues

   # Validate file syntax separately
   node --check suspicious-file.js
   ```

3. **"Memory allocation" or "Stack overflow" errors**:
   ```bash
   # Increase Node.js memory limit
   node --max-old-space-size=8192 ./node_modules/.bin/rolldown build

   # Debug memory usage patterns
   RD_LOG='rolldown_core=debug' node --expose-gc ./node_modules/.bin/rolldown build

   # Check for circular dependencies
   RD_LOG='rolldown_core::chunk_graph=trace' rolldown build | grep -i circular
   ```

4. **"Plugin hook error" messages**:
   ```bash
   # Identify the problematic plugin
   RD_LOG='rolldown_plugin=debug' rolldown build

   # Test plugins individually
   # Remove plugins one by one from config

   # Check plugin hook execution order
   RD_LOG='rolldown_plugin::hook_runner=trace' rolldown build
   ```

5. **"Output generation failed" errors**:
   ```bash
   # Debug chunk generation
   RD_LOG='rolldown_core::chunk_graph=debug' rolldown build

   # Check for conflicting output options
   # Verify output directory permissions
   ls -la output/directory/

   # Test with minimal output configuration
   rolldown build --format es --dir dist-minimal
   ```

6. **Performance degradation patterns**:
   ```bash
   # Profile build timing
   RD_LOG=debug RD_LOG_OUTPUT=chrome-json rolldown build

   # Common performance issues:
   # - Too many small files (consider bundling)
   # - Inefficient plugin hooks (use filters)
   # - Large dependency trees (check for duplicates)
   # - Source map generation overhead

   # Compare with previous builds
   time rolldown build --no-sourcemap
   time rolldown build --sourcemap
   ```

## Maintenance and Prevention

### 21. Keeping Debug Tools Updated

Regularly maintain debugging capabilities:

1. **Update trace logging** in new code areas
2. **Add tests for resolved issues** to prevent regressions
3. **Document new debugging techniques** discovered
4. **Keep benchmark suites** updated for performance monitoring

### 22. Knowledge Sharing

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
