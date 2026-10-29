import defaultValue, { value } from './dep.js';
import assertDefault from 'node:assert';
import './other.js';
import aliasDefault, { value as alias } from './dep.js';
import assertAgain from 'node:assert';
import * as assertNamespace from 'node:assert';

export function read() {
  return {
    defaultValue,
    aliasDefault,
    value,
    alias,
    sameDefault: assertDefault === assertAgain,
    sameNamespace: assertDefault === assertNamespace.default,
  };
}
