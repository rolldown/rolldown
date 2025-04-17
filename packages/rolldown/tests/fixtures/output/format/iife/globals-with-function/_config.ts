import { defineTest } from 'rolldown-tests'
import { expect } from 'vitest'

export default defineTest({
  config: {
    external: /node:path/,
    output: {
      format: 'iife',
      name: 'module',
      globals: (name: string): string => {
        if (name === 'node:path') {
          return 'path'
        }

        return ''
      },
    },
  },
  afterTest: (output) => {
    expect(output.output[0].code).toMatchInlineSnapshot(`
      "var module = (function(node_path) {

      "use strict";
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

      node_path = __toESM(node_path);

      //#region main.js
      var main_default = node_path.join;

      return main_default;
      })(path);"
    `)
  },
})
