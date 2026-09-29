import { useApp } from './app.js';
import info from './info.json' with { type: 'json' };

export default () => useApp() + info.name;
