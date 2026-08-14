// https://github.com/rolldown/rolldown/issues/10186
// Mirrors a Vue SFC compiling `<img src="/favicon.ico">` to `import icon from '/favicon.ico'`.
// A plugin (see _config.ts) keeps the leading-slash id verbatim as the module id.
import icon from '/favicon.ico';

export const src = icon;
