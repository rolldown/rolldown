import { defineTest } from '@tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    external: /node:path/,
    output: {
      exports: 'named',
      format: 'iife',
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code).toMatchInlineSnapshot(`
      "(function(exports, node_path) {

      "use strict";
      Object.defineProperty(exports, '__esModule', { value: true });
      //#region rolldown:runtime
      var __create = Object.create;
      var __defProp = Object.defineProperty;
      var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
      var __getOwnPropNames = Object.getOwnPropertyNames;
      var __getProtoOf = Object.getPrototypeOf;
      var __hasOwnProp = Object.prototype.hasOwnProperty;
      var __copyProps = (to, from, except, desc) => {
      	if (from && typeof from === "object" || typeof from === "function") for (var keys = __getOwnPropNames(from), i = 0, n = keys.length, key; i < n; i++) {
      		key = keys[i];
      		if (!__hasOwnProp.call(to, key) && key !== except) __defProp(to, key, {
      			get: ((k) => from[k]).bind(null, key),
      			enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable
      		});
      	}
      	return to;
      };
      var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", {
      	value: mod,
      	enumerable: true
      }) : target, mod));

      //#endregion
      node_path = __toESM(node_path);

      //#region main.js
      var main_default = node_path.join;

      //#endregion
      Object.defineProperty(exports, 'default', {
        enumerable: true,
        get: function () {
          return main_default;
        }
      });
      return exports;
      })({}, node_path);"
    `)
  },
})
