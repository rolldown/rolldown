import { custom_render } from './flag.js';
import { mount_client } from './main-client.js';
import { chained } from './chain-flag.js';
import { live, enable_live } from './live-flag.js';
import { cross } from './cross-flag.js';
import './cross-user.js';
import * as namespace_flag from './namespace-flag.js';
import { namespace } from './namespace-flag.js';
import { entry_export } from './entry-export-flag.js';
import { dynamic } from './dynamic-flag.js';
import { top } from './top-flag.js';
import { legacy } from './var-flag.js';

export function mount(target) {
  if (custom_render) return mount_client(target);
  throw new Error('server');
}

export const read_chained = () => (chained ? 'chained' : 'not chained');

export function read_live() {
  enable_live();
  return live;
}

export const read_cross = () => cross;
export const get_namespace = () => namespace_flag;
export const read_namespace = () => namespace;
export const read_entry_export = () => entry_export;
export const read_dynamic = () => dynamic;
export const load_dynamic = () => import('./dynamic-flag.js');
export const read_top = () => top;
export const read_legacy = () => legacy;
