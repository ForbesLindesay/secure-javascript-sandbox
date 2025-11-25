export async function evaluate(script, args) {
  let fn
  try {
    fn = new Function(`return (${script})`)()
  } catch {
    throw 'Syntax error in script'
  }
  try {
    const result = await fn(...args.map(arg => JSON.parse(arg)))
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