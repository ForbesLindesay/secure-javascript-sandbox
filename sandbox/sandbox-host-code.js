import {
  stripTypes,
  stripTypesAndCompileModule,
  compileModule,
} from "local:ts-utils/ts-utils-impl";
import { resolveImportPath, loadImport } from "local:host/host-impl";

async function output(fn) {
  try {
    const result = await fn();
    console.log(`73914D86-55DF-495D-BAD5-B45D571D154D`);
    console.log(JSON.stringify(result) ?? "null");
    console.log(`8C47F950-3E81-46B1-976E-177A89380038`);
  } catch (error) {
    console.error(`E8FEE14A-BBF5-4B08-9E00-6D61189D897D`);
    console.error(formatError(error));
  }
}
export async function evaluateModule(code, method, args, options) {
  // TODO: handle cycles
  const moduleCache = new Map();
  async function $import(modulePath, parent, parents) {
    const resolved = await resolveImportPath(modulePath, parent);
    const id = resolved.val;
    if (parents.includes(id)) {
      throw new Error(
        `Circular module dependencies are not yet supported by the sandbox: ${[...parents, id].join(" -> ")}`,
      );
    }
    if (moduleCache.has(id)) {
      return moduleCache.get(id);
    }
    const modulePromise = Promise.resolve().then(async () => {
      const moduleSource =
        resolved.tag === "id"
          ? await loadImport(id)
          : resolved.tag === "url"
            ? await fetch(id).then(async res => {
                if (!res.ok) {
                  throw new Error(
                    `Failed to load module from URL: ${resolved.val}, status: ${res.status}: ${await res.text()}`,
                  );
                }
                return await res.text();
              })
            : (() => {
                throw new Error("Unexpected tag");
              })();
      return await evaluateCompiledModule(
        compileModule(moduleSource, id),
        id,
        parents.concat([id]),
      );
    });
    moduleCache.set(id, modulePromise);
    return modulePromise;
  }

  async function evaluateCompiledModule(compiled, moduleName, parents) {
    let fn;
    try {
      fn = new Function(
        `return (${compiled.code})\n//# sourceURL=${moduleName.replace(/\n/g, "")}`,
      )();
    } catch {
      throw new Error(`Syntax error in module: ${moduleName}`);
    }
    const dependencies = await Promise.all(
      compiled.staticImports.map(async ({ source, names }) => {
        const dep = await $import(source, moduleName, parents);
        for (const name of names) {
          if (!(name in dep)) {
            throw new Error(
              `Module ${moduleName} tried to import '${name}' from module '${source}', but it does not export that name.`,
            );
          }
        }
        return dep;
      }),
    );
    if (compiled.hasDynamicImport) {
      dependencies.unshift(path => $import(path, moduleName, parents));
    }
    return await fn(...dependencies);
  }

  await output(async () => {
    const compiled = options.stripTypes
      ? stripTypesAndCompileModule(code, options.filename)
      : compileModule(code, options.filename);
    const module = await evaluateCompiledModule(
      compiled,
      options.filename ?? "<main>",
      [],
    );
    const fn = module[method];
    return await fn(...args.map(arg => JSON.parse(arg)));
  });
}

export async function evaluate(code, args, options) {
  await output(async () => {
    const compiled = options.stripTypes
      ? stripTypes(`(${code})`, options.filename)
      : `(${code})`;
    let fn;
    try {
      fn = new Function(
        `return ${compiled}\n//# sourceURL=${(options.filename ?? "input.js").replace(/\n/g, "")}`,
      )();
    } catch {
      throw new Error(`Syntax error in function code`);
    }
    return await fn(...args.map(arg => JSON.parse(arg)));
  });
}

function formatError(error) {
  if (!error) {
    return "Unknown error";
  }
  if (typeof error === "string") {
    return error;
  }
  if (
    typeof error.message === "string" &&
    error.stack &&
    !`${error.stack}`.includes(error.message)
  ) {
    return `${error.message}\n${error.stack}`;
  }
  return `${error.stack || error.message || error}`;
}
