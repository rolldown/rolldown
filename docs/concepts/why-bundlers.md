# Why do we still need bundlers?

## Skipping the build step is impractical

With the general availability of native ES modules and HTTP/2 in modern browsers, some developers are advocating for an unbundled approach for shipping web applications, even in production. While this approach works for smaller applications, in our opinion bundling is still very much necessary if you are shipping anything non-trivial and care about performance (which translates to better user experience).

Even in a polished unbundled deployment model, a build step is still often unavoidable. Take Rails 8's default import-map-based approach for example: all JavaScript assets still go through a build step in order to fingerprint the assets and generate the import map and modulepreload directives. It's just handled via `importmap-rails` and Propshaft instead of a JavaScript bundler.

Moreover, the unbundled approach will hit its limits if you have any of the following requirements:

- Require modern JavaScript features like ES6+, TypeScript, or JSX.
- Need to leverage bundler-specific optimizations like tree-shaking, code splitting, or minification.
- Utilize libraries or frameworks that depend on a build step.
- Utilize NPM dependencies that ship unbundled source code (results in too many requests).

Going with unbundled means locking yourself out of a big part of the JS ecosystem and giving up on many possible performance optimizations that could benefit your end users.

The main argument of avoiding JavaScript bundlers is added complexity and slowing down the dev feedback loop. However, modern JS tooling has improved a lot on this front over the past few years. Our goal with Vite / Rolldown is to improve these aspects further and make the build step feel invisible.

## The case for bundlers

Fundamentally, bundlers exist because of the unique constraints of web applications: they need to be delivered over the network on-demand. Bundlers can make web applications more performant in three ways:

1. Reduce the amount of network requests and waterfalls.
2. Reduce total bytes sent over the network.
3. Improve JavaScript execution performance.

## Reduce network requests and waterfalls

The first important thing we need to acknowledge is that **HTTP/2 does not mean you can stop caring about number of HTTP requests**.

Although HTTP/2 theoretically supports unlimited multiplexing, most browsers / servers have a default limit of around 100 on the maximum number of concurrent streams per connection. Every network request also comes with fixed overhead (header processing, TLS encryption, multiplexing, etc.) on both the server and the client. More requests means more server load, and the actual concurrency is limited by how fast your server can serve the module files. Applications that contain thousands of unbundled modules will still create serious network bottlenecks even under HTTP/2.

Deep import chains also results in network waterfalls - i.e. the browser needs to make multiple network roundtrips to fetch the entire module graph. This can be mitigated to some extent with `modulepreload` directives, but generating these requires tooling support, and bloating the HTML with thousands of `modulepreload` directives in `<head>` is also a performance issue in itself.

Bundling can drastically reduce such overhead by combining thousands of modules into an optimal number of chunks that both the server and the browser can handle with ease. Bundling also flattens the import chain depth to reduce waterfalls, and can provide the data needed to generate `modulepreload` directives. In its essence, bundling moves the work of combining the module graph to the build phase, instead of incurring it as a runtime cost for every visitor. This makes large applications load significantly faster on initial visit, especially in poor network conditions.

### Trade-offs in caching strategy

One argument supporting the unbundled approach is that it allows each module to be cached individually, reducing the amount of cache invalidation when the application is updated. However, this comes with the trade-off of a much slower initial load as explained above.

Sub-optimal bundling configurations can cause cascading chunk hash validations, causing users to have to re-download a large part of the app when the app is updated. But this is a solvable problem: bundlers can also leverage import maps and advanced chunking control to limit hash invalidation and improve cache hit rate. We do intend to provide an improved, more caching-friendly default chunking strategy in Vite / Rolldown in the future.

## Reduce total bytes sent over the network

Bundling can also greatly reduce overall size of JavaScript sent over the wire.

First, bundles can hoist multiple modules into the same scope, removing all the import / export statements between them.

Second, treeshaking / dead code elimination is an optimization that can only be performed by statically analyzing the source code at build time. Native ESM loads and evaluates everything eagerly, so even if you only use a single export from a big module, the entire module has to be downloaded and evaluated. With a smart bundler, exports that are not used can be completely removed from the final bundle, saving lots of bytes.

Finally, minification and gzip / brotli compression are considerably more efficient when performed on bundled code compared to individual modules.

With these factors combined, users download less code, and your servers use less outbound bandwidth.

## Improve JavaScript execution performance

JavaScript is an interpreted language, and modern JavaScript engines often employ advanced JIT compilation to make it run faster. However, there is also non-trivial cost involved in parsing and compiling JavaScript.

Sending less JavaScript code not only saves bandwidth - it also means less JavaScript needs to be compiled and evaluated in the browser, leading to faster application startup time.

Some bundlers / minifiers also can perform optimizations like constant folding / ahead-of-time evaluation to varying extent, making the bundled code more efficient than their hand-written source.

---

In conclusion, bundling is still a beneficial, and in many cases necessary step in web development, and will continue to be so in the foreseeable future.
