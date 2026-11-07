import fs from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const dir = new URL('./', import.meta.url);
const root = new URL('../../../', dir);
const binary = fileURLToPath(new URL('gomoku/target/release/examples/diagnose_g4', root));
const position = fileURLToPath(new URL('position.json', dir));
const jobs = [['validate'], ...['e7', 'f6'].map(m => ['line', m]),
  ...['e7','f6'].flatMap(m => [[1000,10],[10000,10],[1000,3],[10000,3],[10000,30]].map(([n,d]) => ['probe',m,String(n),String(d)])),
  ...['g4','f3','h4'].map(m => ['probe',m,'1000','10']),
  ...['e7','f6'].map(m => ['guard',m])];
const records = [];
for (const args of jobs) {
  const start = performance.now();
  const child = spawnSync(binary, [position, ...args], { encoding: 'utf8', timeout: 10000, killSignal: 'SIGKILL' });
  const record = { args, wall_ms: performance.now()-start, status: child.status,
    signal: child.signal, error: child.error?.message ?? null, stdout: child.stdout, stderr: child.stderr };
  records.push(record);
  fs.writeFileSync(new URL('results.json', dir), JSON.stringify(records,null,2)+'\n');
  console.log(`\n$ diagnose_g4 position.json ${args.join(' ')}\n${child.stdout}${child.stderr ?? ''}wall_ms=${record.wall_ms.toFixed(3)} status=${record.status} signal=${record.signal} error=${record.error}`);
}
