import fs from 'node:fs';
import assert from 'node:assert/strict';
const dir = new URL('./', import.meta.url);
const text = fs.readFileSync(new URL('terminal.txt', dir), 'utf8');
const b = [], w = [];
let rows = 0;
for (const line of text.split('\n')) {
  const match = line.match(/^\s*(\d+) \|([^|]+)\|/);
  if (!match) continue;
  const y = Number(match[1]) - 1;
  assert.equal(y, rows++);
  const cells = match[2].trim().split(/\s+/).map(s => s.replace(/[\[\]<>]/g, ''));
  assert.equal(cells.length, 15);
  cells.forEach((cell, x) => {
    assert.ok(['.', '+', 'x', 'o'].includes(cell));
    if (cell === 'x') b.push([x, y]);
    if (cell === 'o') w.push([x, y]);
  });
}
assert.equal(rows, 15);
assert.equal(b.length, 21);
assert.equal(w.length, 21);
const pack = { v: 1, count: 1, items: [{ i: 0, a: 'black', d: 0, b, w, l: [] }] };
fs.writeFileSync(new URL('position.json', dir), JSON.stringify(pack, null, 2) + '\n');
console.log('Parsed 15 rows, 21 Black / 21 White; x=column, y=row, zero-based; no solution claimed.');
