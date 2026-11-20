import lib from 'lib';
import 'polyfill';

(globalThis.externalOrder ??= []).push('app');
globalThis.lib = lib;
