import { custom_render } from './flag.js';

export const render_mode = () => (custom_render ? 'custom' : 'default');
export { enable_entry_export } from './entry-export-flag.js';
