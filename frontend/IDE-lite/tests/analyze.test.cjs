// Ejecuta el handler real del IDE con respuestas HTTP controladas.
// node --test frontend/IDE-lite/tests/analyze.test.cjs
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const source = fs.readFileSync(path.join(__dirname, '..', 'app.jsx'), 'utf8');
const handler = source.match(/const handleRun = async \(\) => \{[\s\S]*?\n  \};/)[0];

for (const database of ['principal', 'qa_otra_base']) {
  test(`ANALIZAR envía la base activa ${database}`, async () => {
    const D = { DATABASE: database, FILES: { sql: { name: 'qa.sql', rawContent: 'SELECT * FROM exclusiva;' } } };
    let request;
    const context = {
      D, API: 'http://localhost:8080', console,
      setLoading() {}, setTab() {}, rerender() {},
      async fetch(url, options) {
        request = { url, body: JSON.parse(options.body) };
        return { ok: true, async json() { return { ok: true, problems: [], tokens: [] }; } };
      },
    };
    await vm.runInNewContext(`${handler}\nhandleRun()`, context);
    assert.equal(request.url, 'http://localhost:8080/api/sql/parse');
    assert.equal(request.body.database, database);
    assert.equal(request.body.script, D.FILES.sql.rawContent);
    assert.equal(D.PARSE_OK, true);
  });
}
