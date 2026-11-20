import * as ns from './lib.js';
import { imported, make, plain } from './lib.js';
import './exported.js';

const local = {
  [Symbol.iterator]() {
    globalThis.calls.push('local');
    return [][Symbol.iterator]();
  },
};

const [fromMake] = make('no side effects');
const [fromPure] = /* @__PURE__ */ plain('pure');
const [fromNamespace] = ns.make('namespace');
const [fromImport] = imported;
let [fromLocal] = local;
const [] = make('empty pattern');
const [, second] = make('hole');
const [...rest] = make('rest element');
const [...[withDefault = globalThis.calls.push('rest default')]] = [];

const [removed, ...removedRest] = [1, 2, 3];
