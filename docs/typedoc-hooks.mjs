// A TypeDoc plugin documenting the hooks of `Plugin` with the signatures of
// `FunctionPluginHooks`: `Plugin` types each hook as `ObjectHook<…>` (the
// function, or an object with it as `handler`), which reads as a type rather
// than as a hook taking parameters.
import { Converter, ReflectionKind } from 'typedoc';

/**
 * The interface of the project with a name.
 *
 * @param {import('typedoc').ProjectReflection} project
 * @param {string} name
 */
const interfaceNamed = (project, name) =>
  project
    .getReflectionsByKind(ReflectionKind.Interface)
    .find((reflection) => reflection.name === name);

/** @param {import('typedoc').Application} app */
export const load = (app) => {
  app.converter.on(Converter.EVENT_RESOLVE_BEGIN, ({ project }) => {
    const hooks = interfaceNamed(project, 'FunctionPluginHooks');

    for (const member of interfaceNamed(project, 'Plugin')?.children ?? []) {
      const hook = hooks?.getChildByName(member.name);

      if (hook) {
        member.type = hook.type;
      }
    }
  });
};
