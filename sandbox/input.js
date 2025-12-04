import { stripTypesOnly, stripTypesAndCompileModule, compileModuleOnly } from "local:ts-utils/ts-utils-impl";

async function output(fn) {
  try {
    const result = await fn()
    console.log(`73914D86-55DF-495D-BAD5-B45D571D154D`)
    console.log(JSON.stringify(result) ?? "null")
    console.log(`8C47F950-3E81-46B1-976E-177A89380038`)
  } catch (error) {
    console.error(`E8FEE14A-BBF5-4B08-9E00-6D61189D897D`)
    console.error(formatError(error))
  }
}
export async function evaluateModule(code, method, args, stripTypes) {
  // TODO: handle cycles
  const moduleCache = new Map()
  async function $import(moduleName) {
    if (moduleCache.has(moduleName)) {
      return moduleCache.get(moduleName)
    }
    const modulePromise = Promise.resolve().then(async () => {
      const moduleResponse = await fetch(moduleName)
      if (!moduleResponse.ok) {
        throw new Error(`Failed to load module: ${moduleName} (${moduleResponse.status} ${moduleResponse.statusText}): ${await moduleResponse.text()}`)
      }
      return await evaluateCompiledModule(compileModuleOnly(await moduleResponse.text()), moduleName)
    })
    moduleCache.set(moduleName, modulePromise)
    return modulePromise
  }

  async function evaluateCompiledModule(compiled, moduleName) {
    let fn
    try {
      fn = new Function(`return (${compiled.code})`)()
    } catch {
      throw new Error(`Syntax error in module: ${moduleName}`)
    }
    const dependencies = await Promise.all(compiled.staticImports.map(moduleName => $import(moduleName)))
    if (compiled.hasDynamicImport) {
      dependencies.unshift($import)
    }
    return await fn(...dependencies)
  }

  await output(async () => {
    const compiled = await stripTypes ? stripTypesAndCompileModule(code) : compileModuleOnly(code);
    const module = await evaluateCompiledModule(compiled, '<main>');
    const fn = module[method];
    return await fn(...args.map(arg => JSON.parse(arg)))
  })
}

export async function evaluate(code, args, stripTypes) {
  await output(async () => {
    const compiled = await stripTypes ? stripTypesOnly(`(${code})`) : `(${code})`;
    let fn
    try {
      fn = new Function(`return ${compiled}`)()
    } catch {
      throw new Error(`Syntax error in function code`)
    }
    return await fn(...args.map(arg => JSON.parse(arg)))
  })
}

function formatError(error) {
  if (!error) {
    return 'Unknown error'
  }
  if (typeof error === 'string') {
    return error
  }
  if (typeof error.message === 'string' && error.stack && !`${error.stack}`.includes(error.message)) {
    return `${error.message}\n${error.stack}`
  }
  return `${error.stack || error.message || error}`
}
