import { identity, readProperty, throughArguments, throughThis } from './lib.js';
import './computed.js';

const value = {
  get state() {
    globalThis.calls.push('getter');
    return 1;
  },
  valueOf() {
    globalThis.calls.push('valueOf');
    return 2;
  },
  toString() {
    globalThis.calls.push('toString');
    return 'value';
  },
};

const read = { default: readProperty(value) };
const number = { default: +identity(value) };
const string = { default: `label: ${identity(value)}` };
const argument = { default: identity(value.state) };
const stringCall = { default: String(identity(value)) };

const byThis = { default: throughThis(() => globalThis.calls.push('this')) };
const byArguments = { default: throughArguments(() => globalThis.calls.push('arguments')) };

function effect() {
  globalThis.calls.push('argument evaluation');
  return 1;
}
const effectfulArgument = { default: identity(effect()) };
