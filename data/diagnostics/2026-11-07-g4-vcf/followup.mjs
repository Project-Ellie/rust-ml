// Only fresh diagnostic children are timed out; no live process interaction.
import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const dir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(dir, '../../..');
const exe = path.join(root, 'gomoku/target/release/examples/diagnose_g4');
const fixture = path.join(dir, 'position.json');
const immutable = ['terminal.txt', 'position.json', 'metadata.json'];
const hashes = () => Object.fromEntries(immutable.map(f => [f, createHash('sha256').update(readFileSync(path.join(dir,f))).digest('hex')]));
const before = hashes();
const runs = [];
function run(args) {
  const start = Date.now();
  const p = spawnSync(exe, [fixture,...args.map(String)], { cwd:root, encoding:'utf8', timeout:10000, killSignal:'SIGKILL', maxBuffer:16*1024*1024 });
  const run = { args, status:p.status, signal:p.signal, error:p.error?.code ?? null, elapsed_ms:Date.now()-start, stdout:p.stdout, stderr:p.stderr };
  run.parsed = (p.stdout ?? '').split('\n').filter(s=>s.startsWith('{')).map(s=>JSON.parse(s));
  runs.push(run);
  console.log(`${args.join(' ')} status=${run.status} signal=${run.signal} error=${run.error} elapsed_ms=${run.elapsed_ms}`);
  writeFileSync(path.join(dir,'followup-results.json'),JSON.stringify({immutable_before:before,runs},null,2)+'\n');
  writeFileSync(path.join(dir,'followup-stdout.txt'),runs.map(r=>`$ diagnose_g4 position.json ${r.args.join(' ')}\n${r.stdout}${r.stderr}\nstatus=${r.status} signal=${r.signal} error=${r.error}\n`).join('\n'));
  return run;
}
const records = [];
for(let start=0; start<183; start+=20) {
  const r = run(['exhaustive',start,Math.min(start+20,183)]);
  if(r.status !== 0) throw Error('exhaustive chunk incomplete; no exhaustive claim allowed');
  records.push(...r.parsed[0].records);
}
const item = JSON.parse(readFileSync(fixture,'utf8')).items[0];
const occupied = new Set([...item.b,...item.w].map(([x,y])=>y*15+x));
const legalIndices = Array.from({length:225},(_,i)=>i).filter(i=>!occupied.has(i));
if(records.length !== 183 || new Set(records.map(x=>x.index)).size !== 183 || JSON.stringify(records.map(x=>x.index)) !== JSON.stringify(legalIndices)) throw Error('missing/duplicate/incorrect candidate');
const own = run(['own']);
const real = run(['real',10000]);
if(real.status !== 0) {
  run(['real',100]);
  run(['real',1000]);
}
const summary = {
  legal_count:183, examined:records.length,
  white_win_certified:records.filter(x=>x.white_win_certified).length,
  black_terminal_win_or_draw:records.filter(x=>x.black_terminal_win_or_draw).length,
  unknown:records.filter(x=>!x.white_win_certified && !x.black_terminal_win_or_draw).map(x=>x.move),
  proof_attempts:records.reduce((n,r)=>n+r.attempts.length,0),
  verified_lines:records.reduce((n,r)=>n+r.attempts.filter(a=>a.proof.verified).length,0),
  admitted_lines:records.reduce((n,r)=>n+r.attempts.filter(a=>a.proof.admitted).length,0),
  actual_tss_calls:records.reduce((n,r)=>n+r.attempts.filter(a=>a.source==='actual_tss').length,0),
  legal_indices_match_fixture:true,
  per_known_line:[0,1,2].map(i=>({line:records[0].attempts[i].proof.line, verified:records.filter(r=>r.attempts[i].proof.verified).length, admitted:records.filter(r=>r.attempts[i].proof.admitted).length})),
  own_probe_completed:own.status===0,
  immutable_unchanged:JSON.stringify(before)===JSON.stringify(hashes())
};
writeFileSync(path.join(dir,'root-certificates.json'),JSON.stringify({summary,records},null,2)+'\n');
writeFileSync(path.join(dir,'followup-results.json'),JSON.stringify({immutable_before:before,immutable_after:hashes(),summary,runs},null,2)+'\n');
console.log(JSON.stringify(summary));
for(const r of runs.filter(r=>r.args[0]==='real')) for(const p of r.parsed) { const {ordering,...short}=p; console.log(JSON.stringify(short)); }
