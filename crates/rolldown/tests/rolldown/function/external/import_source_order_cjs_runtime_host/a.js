import ext from 'ext';
import shared from './shared.cjs';

globalThis.values = [ext.value, shared.value];
