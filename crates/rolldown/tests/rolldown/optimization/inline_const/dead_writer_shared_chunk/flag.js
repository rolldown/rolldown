// Issue #9698: only a module that is not in the graph calls the writer.
export let custom_render = false;

export function enable_custom_render() {
  custom_render = true;
}
