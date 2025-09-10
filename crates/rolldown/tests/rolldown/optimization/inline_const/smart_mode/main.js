import {
  mode,
  one,
  bool,
  primitiveNull,
  primitiveUndefined,
  longString,
  shortString
} from './constants.js';


console.log(mode, one, bool, primitiveNull, primitiveUndefined, longString, shortString);

if (one || mode || bool || primitiveNull || primitiveUndefined || longString || shortString) {
  console.log('test')
}

if (process.env.NODE_ENV === mode) {
  console.log('production')
}

export function test() {
  var two = mode === 1 ? 'two' : 'unused two';
  var three = mode === 1 || 'unused three';
  var four = mode === 1 && 'four';
  var five = mode ?? 'five';
  return two + three + four + five;
}

