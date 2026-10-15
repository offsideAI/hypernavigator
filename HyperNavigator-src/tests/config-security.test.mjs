// Static audit of HyperNavigator's security-relevant configuration. Run with `npm test`.
// Complements the Rust ACL tests (src-tauri/src/lib.rs `acl_tests`) and the runtime probe.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const root = join(import.meta.dirname, '..');
const read = (path) => readFileSync(join(root, path), 'utf8');
const json = (path) => JSON.parse(read(path));

const buildRsCommands = () => [...read('src-tauri/build.rs').matchAll(/^\s+"([a-z_]+)",$/gm)].map((m) => m[1]);

test('tauri.conf.json keeps the Tauri JS API off the window object and sets a CSP', () => {
  const conf = json('src-tauri/tauri.conf.json');
  assert.equal(conf.app.withGlobalTauri, false);
  assert.deepEqual(conf.app.windows, [], 'windows are created in Rust so the child WebViews get explicit labels');
  assert.deepEqual(conf.app.security.capabilities, ['chrome', 'guest']);
  const csp = conf.app.security.csp;
  assert.equal(csp['default-src'], "'self'");
  assert.equal(csp['script-src'], "'self'");
  assert.equal(csp['frame-src'], "'none'", 'no iframes in the trusted chrome');
});

test('chrome capability is bound to the chrome WebView label, local content only', () => {
  const cap = json('src-tauri/capabilities/chrome.json');
  assert.deepEqual(cap.webviews, ['chrome']);
  assert.equal(cap.windows, undefined, 'a window-scoped capability would also cover the guest');
  assert.equal(cap.remote, undefined, 'no remote URLs may use the chrome capability');
  assert.equal(cap.local, true);
  for (const permission of cap.permissions) {
    assert.ok(typeof permission === 'string', 'no scoped permission objects');
    assert.doesNotMatch(permission, /default|allow-all|\*/, `broad permission ${permission}`);
    assert.doesNotMatch(permission, /^(fs|shell|http|process|opener|dialog):/, `plugin permission ${permission}`);
  }
});

test('guest capability grants nothing', () => {
  const cap = json('src-tauri/capabilities/guest.json');
  assert.deepEqual(cap.webviews, ['guest-*']);
  assert.deepEqual(cap.permissions, []);
  assert.equal(cap.remote, undefined);
});

test('build.rs, the invoke handler and chrome.json list exactly the same commands', () => {
  const commands = buildRsCommands();
  assert.ok(commands.length > 0);
  const handler = read('src-tauri/src/lib.rs').match(/generate_handler!\[([\s\S]*?)\]/)[1];
  const registered = [...handler.matchAll(/commands::([a-z_]+)/g)].map((m) => m[1]);
  assert.deepEqual([...registered].sort(), [...commands].sort());
  const allowed = json('src-tauri/capabilities/chrome.json')
    .permissions.filter((p) => p.startsWith('allow-'))
    .map((p) => p.slice('allow-'.length).replaceAll('-', '_'));
  assert.deepEqual(allowed.sort(), [...commands].sort());
});

test('no Rust command exposes generic filesystem, shell or URL-open access', () => {
  const source = read('src-tauri/src/commands.rs');
  assert.doesNotMatch(source, /std::process|Command::new|read_to_string|fs::read\b|open::that|shell/);
  // Folder sharing only accepts a request id and a yes/no decision from the chrome, never a path.
  assert.match(source, /resolve_share_folder<R: Runtime>\(app: AppHandle<R>, request_id: u64, approve: bool\)/);
});

test('trusted chrome code never relies on a global window.__TAURI__', () => {
  for (const file of readdirSync(join(root, 'src')).filter((f) => f.endsWith('.ts'))) {
    assert.doesNotMatch(read(`src/${file}`), /__TAURI__/, file);
  }
});

test('the browser builds without files from the examples project', () => {
  // Examples live in ../WebAppExamples-src and only reach the browser over HTTP.
  const buildInputs = ['package.json', 'vite.config.ts', 'src-tauri/Cargo.toml', 'src-tauri/build.rs'];
  for (const dir of ['src', 'src-tauri/src']) {
    for (const file of readdirSync(join(root, dir))) buildInputs.push(`${dir}/${file}`);
  }
  for (const file of buildInputs) {
    assert.doesNotMatch(read(file), /include_str!\("\.\.\/\.\.\/\.\.|examples\/counter|\.\.\/WebAppExamples-src/, file);
  }
});

test('product-facing files never use the old DotWeb name', () => {
  const files = [
    'index.html',
    'placeholder.html',
    'src/main.ts',
    'src-tauri/tauri.conf.json',
    'README.md',
  ];
  for (const file of files) assert.doesNotMatch(read(file), /dotweb/i, file);
});
