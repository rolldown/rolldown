/* cSpell:disable */

/// Node.js built-in modules
///
/// `node -p "[...require('module').builtinModules].map(b => JSON.stringify(b)).join(',\n')"`
/// <https://nodejs.org/api/modules.html#core-modules>
/// copy from `oxc_resolver`
pub const NODEJS_BUILTINS: &[&str] = &[
  "_http_agent",
  "_http_client",
  "_http_common",
  "_http_incoming",
  "_http_outgoing",
  "_http_server",
  "_stream_duplex",
  "_stream_passthrough",
  "_stream_readable",
  "_stream_transform",
  "_stream_wrap",
  "_stream_writable",
  "_tls_common",
  "_tls_wrap",
  "assert",
  "assert/strict",
  "async_hooks",
  "buffer",
  "child_process",
  "cluster",
  "console",
  "constants",
  "crypto",
  "dgram",
  "diagnostics_channel",
  "dns",
  "dns/promises",
  "domain",
  "events",
  "fs",
  "fs/promises",
  "http",
  "http2",
  "https",
  "inspector",
  "module",
  "net",
  "os",
  "path",
  "path/posix",
  "path/win32",
  "perf_hooks",
  "process",
  "punycode",
  "querystring",
  "readline",
  "repl",
  "stream",
  "stream/consumers",
  "stream/promises",
  "stream/web",
  "string_decoder",
  "sys",
  "timers",
  "timers/promises",
  "tls",
  "trace_events",
  "tty",
  "url",
  "util",
  "util/types",
  "v8",
  "vm",
  "worker_threads",
  "zlib",
];

/// Using `phf` should be faster, but it would increase the compile time, since this function is
/// not frequently used, we use `binary_search` instead.
pub fn is_builtin_modules(specifier: &str) -> bool {
  specifier.starts_with("node:") || NODEJS_BUILTINS.binary_search(&specifier).is_ok()
}

#[test]
fn test_is_builtin_modules() {
  // not prefix-only modules
  assert!(is_builtin_modules("fs"));
  assert!(is_builtin_modules("node:fs"));
  // prefix-only modules
  assert!(is_builtin_modules("node:test"));
  // not a builtin module
  assert!(!is_builtin_modules("unknown"));
}
