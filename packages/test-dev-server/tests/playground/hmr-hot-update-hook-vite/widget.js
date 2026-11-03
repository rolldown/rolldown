// Mimics the SFC pattern (plugin-vue): a base file whose query-variant
// sub-module derives its content from the base file. The plugin's load()
// for `widget.js?part=extra` reads the marker below.
// part-marker: part-v1
import './widget.js?part=extra';

window.__widgetRuns = (window.__widgetRuns ?? 0) + 1;
