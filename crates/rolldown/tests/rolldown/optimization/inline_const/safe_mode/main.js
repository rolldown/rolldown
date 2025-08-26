import { 
  mode,
  one
} from './constants.js';


if (process.env.NODE_ENV === mode) {
  console.log('Production mode code');
}

console.log(mode, one)


export var mode_ident = mode;
export var o = one;
export var two = one === 1 ? 'two' : 'unused two';
export var three = one === 1 || 'unused three';
export var four = one === 1 && 'four';
export var five = undefined ?? 'five';
