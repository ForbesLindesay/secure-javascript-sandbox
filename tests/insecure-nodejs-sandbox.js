// This "sandbox" is relatively easy to escape, and does not
// do anything to prevent excessive CPU and RAM usage

const {Worker, isMainThread, workerData, parentPort} = require("worker_threads")
const vm = require("vm")


if (isMainThread) {
  const script = arg(`--script`)
  const args = arg(`--args`)
  const endpoint = arg(`--endpoint`)
  const repeat = parseInt(arg(`--repeat`) ?? `1`, 10)
  const threads = parseInt(arg(`--threads`) ?? `1`, 10)
  const timeoutMs = parseInt(arg(`--timeout-ms`) ?? `1000`, 10)
  const quiet = process.argv.includes(`--quiet`)

  const runOnce = endpoint ? getRunForEndpoint(script, args, endpoint) : () => runWorker(script, args, timeoutMs)
  const run = async (repeat) => {
    for (let i = 0; i < repeat; i++) {
      const start = Date.now()
      const result = await runOnce()
      const end = Date.now()
      if (result !== 377) {
        throw new Error(`Unexpected result: ${result}`)
      }
      if (!quiet) {
        console.warn(`${end - start}ms`)
        console.log(result)
      }
    }
  }
  if (threads > 1) {
    const perThread = Math.ceil(repeat / threads)
    const threadResults = []
    let repeatRemaining = repeat
    while (repeatRemaining > 0) {
      const currentThreadRepeat = Math.min(perThread, repeatRemaining)
      repeatRemaining -= currentThreadRepeat
      threadResults.push(run(currentThreadRepeat))
    }
    Promise.all(threadResults).catch(ex => {
      console.error(ex.stack)
      process.exit(1)
    })
  } else {
    run(repeat).catch(ex => {
      console.error(ex.stack)
      process.exit(1)
    })
  }
} else {
  const result = vm.runInNewContext(`(${workerData.script})(...${workerData.args})`)
  parentPort.postMessage(result)
}

function arg(name) {
  const idx = process.argv.indexOf(name)
  return idx === -1 ? undefined : process.argv[idx + 1]
}
function getRunForEndpoint(script, args, endpoint) {
  const body = JSON.stringify({ code: script, parameters: JSON.parse(args) })
  return async () => {
    const res = await fetch(endpoint, {
      method: 'POST',
      headers: {"Content-Type": "application/json"},
      body
    })
    if (!res.ok) {
      throw new Error(`Failed to run script: ${res.status} ${res.statusText}: ${await res.text()}`)
    }
    return (await res.json()).result
  }
}
async function runWorker(script, args, timeoutMs, endpoint) {
  if (endpoint) {
    const res = await fetch(endpoint, {
      method: 'POST',
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({ script, args: JSON.parse(args) })
    })
    if (!res.ok) {
      throw new Error(`Failed to run script: ${res.status} ${res.statusText}: ${await res.text()}`)
    }
    return res.json().result
  }
  const worker = (new Worker(__filename, { workerData: { script, args }}));
  let result
  let timedOut = false
  const timeout = setTimeout(() => {
    timedOut = true
    worker.terminate()
  }, timeoutMs)
  const exitCode = await new Promise((resolve, reject) => {
    worker.on('message', (msg) => {
      result = msg
    });
    worker.on('error', reject);
    worker.on('exit', resolve);
  })
  clearTimeout(timeout)
  if (timedOut) {
    throw new Error(`Timeout exceeded`)
  }
  if (exitCode !== 0) {
    throw new Error(`Exited with code ${exitCode}`)
  }
  return result
}