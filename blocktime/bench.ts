import { ApiPromise, WsProvider } from "@polkadot/api";
import fs from "fs";
import os from "os";
import path from "path";

async function main() {
  const provider = new WsProvider("ws://127.0.0.1:9944");
  const api = await ApiPromise.create({
    provider,
  });
  let last_block_time = unix_seconds();
  let blocktimes: number[] = [];
  const unsubscribe = await api.rpc.chain.subscribeNewHeads(() => {
    const now = unix_seconds();
    const new_time = now - last_block_time;
    console.log(`blocktime: ${new_time}s\n`);
    blocktimes.push(new_time);
    last_block_time = now;
  });

  console.log(`block times: ${blocktimes}\n\n`);

  const interval = setInterval(() => {
    
    if (blocktimes.length >= 21) {
      blocktimes.shift();
      const lines = blocktimes.join("\n");
      const cpus = os.cpus();
      const platform = os.platform();
      const mem = os.totalmem() / 1000000000;
      const systemInfo = `${platform}\n${cpus[0].model}\n ${cpus.length} cores\nTotal memory: ${mem}gb`;
      const avgBlockTime = blocktimes.reduce((acc, t) => acc + t, 0) / blocktimes.length
      fs.writeFileSync(
        path.join(__dirname, `/mining_worker_benchmark_${Date.now()}`),
        `${systemInfo}\nDifficulty:1_000_000_000\nBlock times(seconds)\n${lines}\nAvg Block time: ${avgBlockTime}`
      );
      unsubscribe();
      clearInterval(interval);
    }
  }, 500);
}

const unix_seconds = () => Date.now() / 1000;

main();
