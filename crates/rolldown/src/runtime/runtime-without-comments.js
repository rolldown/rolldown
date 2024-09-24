
var __create = Object.create
var __defProp = Object.defineProperty
var __getOwnPropDesc = Object.getOwnPropertyDescriptor
var __getOwnPropNames = Object.getOwnPropertyNames
var __getProtoOf = Object.getPrototypeOf
var __hasOwnProp = Object.prototype.hasOwnProperty
var __esm = (fn, res) => function () {
  return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res
}
var __esmMin = (fn, res) => () => (fn && (res = fn(fn = 0)), res)
var __commonJS = (cb, mod) => function () {
  return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports
}
var __commonJSMin = (cb, mod) => () => (mod || cb((mod = { exports: {} }).exports, mod), mod.exports)
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
var __reExport = (target, mod, secondTarget) => (
  __copyProps(target, mod, 'default'),
  secondTarget && __copyProps(secondTarget, mod, 'default')
)
var __toESM = (mod, isNodeMode, target) => (
  target = mod != null ? __create(__getProtoOf(mod)) : {},
  __copyProps(
    isNodeMode || !mod || !mod.__esModule
      ? __defProp(target, 'default', { value: mod, enumerable: true })
      : target,
    mod)
)
var __toCommonJS = mod => __copyProps(__defProp({}, '__esModule', { value: true }), mod)
var __toBinaryNode = base64 => new Uint8Array(Buffer.from(base64, 'base64'))
var __toBinary = /* @__PURE__ */ (() => {
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

var rolldown_runtime = self.rolldown_runtime = {
  patching: false,
  patchedModuleFactoryMap: {},
  executeModuleStack: [],
  moduleCache: {},
  moduleFactoryMap: {},
  define: function (id, factory) {
    if (self.patching) {
      this.patchedModuleFactoryMap[id] = factory;
    } else {
      this.moduleFactoryMap[id] = factory;
    }
  },
  require: function (id) {
    const parent = this.executeModuleStack.length > 1 ? this.executeModuleStack[this.executeModuleStack.length - 1] : null;
    if (this.moduleCache[id]) {
      var module = this.moduleCache[id];
      if(module.parents.indexOf(parent) === -1) {
        module.parents.push(parent);
      }
      return module.exports;
    }
    var factory = this.moduleFactoryMap[id];
    if (!factory) {
      throw new Error('Module not found: ' + id);
    }
    var module = this.moduleCache[id] = { 
      exports: {},
      parents: [parent],
      hot: {
        selfAccept: false,
        acceptCallbacks: [],
        accept: function(callback) {
          this.selfAccept = true;
          if(callback && typeof callback === 'function') {
            this.acceptCallbacks.push({
              deps: [id],
              callback
            });
          }
        }
      }
    };
    this.executeModuleStack.push(id);
    factory(this.require.bind(this), module, module.exports);
    this.executeModuleStack.pop();
    return module.exports;
  },
  patch: function(updateModuleIds, callback) {
    self.patching = true;

    callback();

    var boundaries = [];
    var invalidModuleIds = [];
    var acceptCallbacks = [];

    for (var i = 0; i < updateModuleIds.length; i++) {
      foundBoundariesAndInvalidModuleIds(updateModuleIds[i], boundaries, invalidModuleIds, acceptCallbacks)
    }

    for (var i = 0; i < invalidModuleIds.length; i++) {
      var id = invalidModuleIds[i];
      delete this.moduleCache[id];
    }

    for (var id in this.patchedModuleFactoryMap) {
      this.moduleFactoryMap[id] = this.patchedModuleFactoryMap[id];
    }
    this.patchedModuleFactoryMap = {}

    for (var i = 0; i < boundaries.length; i++) {
      this.require(boundaries[i]);
    }

    for (var i = 0; i < acceptCallbacks.length; i++) {
      var item = acceptCallbacks[i];
      item.callback.apply(null, item.deps.map((dep) => this.moduleCache[dep].exports));
    }

    self.patching = false;

    function foundBoundariesAndInvalidModuleIds(updateModuleId, boundaries, invalidModuleIds, acceptCallbacks) {
      var queue = [ { moduleId: updateModuleId, chain: [updateModuleId] }];
      var visited = {};
     
      while (queue.length > 0) {
        var item = queue.pop();
        var moduleId = item.moduleId;
        var chain = item.chain;

        if (visited[moduleId]) {
          continue;
        }

        var module = rolldown_runtime.moduleCache[moduleId];

        if (module.hot.selfAccept) {
          if(boundaries.indexOf(moduleId) === -1) {
            boundaries.push(moduleId);

            for (var i = 0; i < module.hot.acceptCallbacks.length; i++) {
              var item = module.hot.acceptCallbacks[i];
              if(item.deps.includes(updateModuleId)) {
                acceptCallbacks.push(item);
              }
            }
          }
          for (var i = 0; i < chain.length; i++) {
            if(invalidModuleIds.indexOf(chain[i]) === -1) {
              invalidModuleIds.push(chain[i]);
            }
          }
          continue;
        }

        for(var i = 0; i < module.parents.length; i++) {
          var parent = module.parents[i];
          queue.push({
            moduleId: parent,
            chain: chain.concat([parent])
          });
        }

        visited[moduleId] = true;
      }


    }
  }
  ,
  loadScript: function (url) {
    var script = document.createElement('script');
    script.src = url;
    script.onerror = function() {
      console.error('Failed to load script: ' + url);
    }    
    document.body.appendChild(script);
  }
}

const socket = new WebSocket(`ws://localhost:8080`)

socket.onmessage = function(event) {
  const data = JSON.parse(event.data)
  if (data.type === 'update') {
    rolldown_runtime.loadScript(data.url)
  }
}
