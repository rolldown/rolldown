// Port from https://github.com/evanw/esbuild/blob/main/internal/runtime/runtime.go
var __create = Object.create
var __defProp = Object.defineProperty
var __getOwnPropDesc = Object.getOwnPropertyDescriptor // Note: can return "undefined" due to a Safari bug
var __getOwnPropNames = Object.getOwnPropertyNames
var __getProtoOf = Object.getPrototypeOf
var __hasOwnProp = Object.prototype.hasOwnProperty
// This is for lazily-initialized ESM code. This has two implementations, a
// compact one for minified code and a verbose one that generates friendly
// names in V8's profiler and in stack traces.
var __esm = (fn, res) => function () {
  return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res
}
var __esmMin = (fn, res) => () => (fn && (res = fn(fn = 0)), res)
// Wraps a CommonJS closure and returns a require() function. This has two
// implementations, a compact one for minified code and a verbose one that
// generates friendly names in V8's profiler and in stack traces.
var __commonJS = (cb, mod) => function () {
  return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports
}
var __commonJSMin = (cb, mod) => () => (mod || cb((mod = { exports: {} }).exports, mod), mod.exports)
// Used to implement ESM exports both for "require()" and "import * as"
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true })
}

var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === 'object' || typeof from === 'function')
    for (var keys = __getOwnPropNames(from), i = 0, n = keys.length, key; i < n; i++) {
      key = keys[i]
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: (k => from[k]).bind(null, key), enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable })
    }
  return to
}

// This is used to implement "export * from" statements. It copies properties
// from the imported module to the current module's ESM export object. If the
// current module is an entry point and the target format is CommonJS, we
// also copy the properties to "module.exports" in addition to our module's
// internal ESM export object.
var __reExport = (target, mod, secondTarget) => (
  __copyProps(target, mod, 'default'),
  secondTarget && __copyProps(secondTarget, mod, 'default')
)

// Converts the module from CommonJS to ESM. When in node mode (i.e. in an
// ".mjs" file, package.json has "type: module", or the "__esModule" export
// in the CommonJS file is falsy or missing), the "default" property is
// overridden to point to the original CommonJS exports object instead.
var __toESM = (mod, isNodeMode, target) => (
  target = mod != null ? __create(__getProtoOf(mod)) : {},
  __copyProps(
    // If the importer is in node compatibility mode or this is not an ESM
    // file that has been converted to a CommonJS file using a Babel-
    // compatible transform (i.e. "__esModule" has not been set), then set
    // "default" to the CommonJS "module.exports" for node compatibility.
    isNodeMode || !mod || !mod.__esModule
      ? __defProp(target, 'default', { value: mod, enumerable: true })
      : target,
    mod)
)

// Converts the module from ESM to CommonJS. This clones the input module
// object with the addition of a non-enumerable "__esModule" property set
// to "true", which overwrites any existing export named "__esModule".
var __toCommonJS = mod => __copyProps(__defProp({}, '__esModule', { value: true }), mod)

// This is for the "binary" loader (custom code is ~2x faster than "atob")
export var __toBinaryNode = base64 => new Uint8Array(Buffer.from(base64, 'base64'))
export var __toBinary = /* @__PURE__ */ (() => {
  var table = new Uint8Array(128)
  for (var i = 0; i < 64; i++) table[i < 26 ? i + 65 : i < 52 ? i + 71 : i < 62 ? i - 4 : i * 4 - 205] = i
  return base64 => {
    var n = base64.length, bytes = new Uint8Array((n - (base64[n - 1] == '=') - (base64[n - 2] == '=')) * 3 / 4 | 0)
    for (var i = 0, j = 0; i < n;) {
      var c0 = table[base64.charCodeAt(i++)], c1 = table[base64.charCodeAt(i++)]
      var c2 = table[base64.charCodeAt(i++)], c3 = table[base64.charCodeAt(i++)]
      bytes[j++] = (c0 << 2) | (c1 >> 4)
      bytes[j++] = (c1 << 4) | (c2 >> 2)
      bytes[j++] = (c2 << 6) | c3
    }
    return bytes
  }
})()
