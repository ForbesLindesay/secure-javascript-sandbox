// TODO: handle cycles
import {striptypesonly} from "local:tsutils";

console.log("striptypesonly=", striptypesonly)

const moduleCache = new Map()
async function $import(moduleName) {
  if (moduleCache.has(moduleName)) {
    return moduleCache.get(moduleName)
  }
  const modulePromise = Promise.resolve().then(async () => {
    const moduleResponse = await fetch(moduleName, {
      headers: {
        "X-COMPILE-MODULE-FOR-SANDBOX": "1"
      }
    })
    if (!moduleResponse.ok) {
      throw new Error(`Failed to load module: ${moduleName} (${moduleResponse.status} ${moduleResponse.statusText}): ${await moduleResponse.text()}`)
    }
    return await evaluateModule(await moduleResponse.json(), moduleName)
  })
  moduleCache.set(moduleName, modulePromise)
  return modulePromise
}
async function evaluateModule(options, moduleName) {
  let fn
  try {
    fn = new Function(`return (${options.code})`)()
  } catch {
    throw new Error(`Syntax error in module: ${moduleName}`)
  }
  const dependencies = await Promise.all(options.static_imports.map(moduleName => $import(moduleName)))
  if (options.has_dynamic_import) {
    dependencies.unshift($import)
  }
  return await fn(...dependencies)
}
export async function evaluate(code, has_dynamic_import, static_imports, method, args) {
  try {
    const module = await evaluateModule({code, has_dynamic_import, static_imports}, '<main>')
    const result = await module[method](...args.map(arg => JSON.parse(arg)))
    return JSON.stringify(result) ?? "null"
  } catch (error) {
    // error.stack may sometimes be defined but set to an empty string.
    throw formatError(error)
  }
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