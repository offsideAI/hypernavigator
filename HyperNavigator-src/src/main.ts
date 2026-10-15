// Trusted browser chrome. All decisions (URL policy, fetching, history, file access) happen in Rust;
// this file only renders state and forwards user intent.
import * as api from './api';
import type { NavState, SharePrompt } from './api';

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const backButton = $<HTMLButtonElement>('back');
const forwardButton = $<HTMLButtonElement>('forward');
const reloadButton = $<HTMLButtonElement>('reload');
const addressForm = $<HTMLFormElement>('address-form');
const addressInput = $<HTMLInputElement>('address');
const goButton = $<HTMLButtonElement>('go');
const shareButton = $<HTMLButtonElement>('share');
const status = $<HTMLSpanElement>('status');
const statusText = $<HTMLSpanElement>('status-text');
const originLabel = $<HTMLSpanElement>('origin');
const dialog = $<HTMLDivElement>('share-dialog');
const chrome = $<HTMLElement>('chrome');

type StatusKind = 'idle' | 'busy' | 'ok' | 'error' | 'warn';

let state: NavState | null = null;
let busy = false;
let guestLoading = false;
let pendingPrompt: SharePrompt | null = null;

function setStatus(kind: StatusKind, text: string) {
  status.dataset.kind = kind;
  statusText.textContent = text;
  status.title = text;
}

function errorText(e: unknown): string {
  return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e);
}

function render() {
  const s = state;
  backButton.disabled = busy || !s?.canGoBack;
  forwardButton.disabled = busy || !s?.canGoForward;
  reloadButton.disabled = busy || !s?.canReload;
  goButton.disabled = busy;
  shareButton.disabled = busy || guestLoading || s?.status !== 'loaded';
  originLabel.textContent = s?.app ? `Guest: ${s.app.origin}` : 'Guest: none';
  originLabel.title = s?.app ? `App entry: ${s.app.entryUrl}` : 'No app loaded';
  if (s?.url && document.activeElement !== addressInput) {
    addressInput.value = s.url;
  }
}

function describe(s: NavState) {
  if (s.status === 'error') {
    setStatus('error', s.message ?? 'Error');
  } else if (s.status === 'loaded' && s.app) {
    const shared = s.sharedFolder
      ? ` - shared ${s.sharedFolder.entries} names from "${s.sharedFolder.folderName}"`
      : '';
    setStatus(guestLoading ? 'busy' : 'ok', `${guestLoading ? 'Loading' : 'Running'} ${s.app.name} ${s.app.version}${shared}`);
  } else {
    setStatus('idle', 'Ready');
  }
}

async function run(label: string, action: () => Promise<NavState>) {
  if (busy) return;
  busy = true;
  render();
  setStatus('busy', label);
  try {
    state = await action();
    guestLoading = state.status === 'loaded';
    describe(state);
  } catch (e) {
    // Rejected before navigating (invalid URL, empty history): the guest is unchanged.
    setStatus('error', errorText(e));
  } finally {
    busy = false;
    render();
  }
}

addressForm.addEventListener('submit', (event) => {
  event.preventDefault();
  void run('Fetching manifest...', () => api.navigate(addressInput.value));
});
backButton.addEventListener('click', () => void run('Going back...', api.goBack));
forwardButton.addEventListener('click', () => void run('Going forward...', api.goForward));
reloadButton.addEventListener('click', () => void run('Reloading...', api.reload));

// --- Share folder names --------------------------------------------------------------------------

function openDialog(prompt: SharePrompt) {
  pendingPrompt = prompt;
  $('share-app').textContent = prompt.appName;
  $('share-origin').textContent = prompt.appOrigin;
  $('share-folder').textContent = prompt.folderName;
  $('share-max').textContent = String(prompt.maxEntries);
  chrome.inert = true;
  dialog.hidden = false;
  $<HTMLButtonElement>('share-cancel').focus();
}

async function closeDialog(approve: boolean) {
  const prompt = pendingPrompt;
  if (!prompt) return;
  pendingPrompt = null;
  dialog.hidden = true;
  chrome.inert = false;
  setStatus('busy', approve ? 'Sharing folder names...' : 'Cancelled');
  try {
    state = await api.resolveShareFolder(prompt.requestId, approve);
    describe(state);
    if (!approve) setStatus('ok', 'Nothing was shared');
  } catch (e) {
    setStatus('error', errorText(e));
  } finally {
    render();
  }
}

shareButton.addEventListener('click', async () => {
  if (busy) return;
  busy = true;
  render();
  setStatus('busy', 'Choose a folder...');
  try {
    const prompt = await api.pickShareFolder();
    if (prompt) {
      openDialog(prompt);
    } else if (state) {
      describe(state);
    }
  } catch (e) {
    setStatus('error', errorText(e));
  } finally {
    busy = false;
    render();
  }
});

$('share-cancel').addEventListener('click', () => void closeDialog(false));
$('share-confirm').addEventListener('click', () => void closeDialog(true));
document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && pendingPrompt) void closeDialog(false);
});

// --- Guest lifecycle -----------------------------------------------------------------------------

void api.onGuestEvent((event) => {
  switch (event.kind) {
    case 'pageLoad':
      guestLoading = !event.finished && state?.status === 'loaded';
      if (state) describe(state);
      break;
    case 'blocked':
      setStatus('warn', `Blocked navigation outside ${'http://127.0.0.1:8765'}: ${event.url}`);
      break;
    case 'leftApp':
      if (state) state = { ...state, sharedFolder: null };
      setStatus('warn', `The app navigated itself to ${event.url}; any folder grant was cleared`);
      break;
  }
  render();
});

addressInput.value = api.DEFAULT_MANIFEST_URL;
api
  .navState()
  .then((s) => {
    state = s;
    describe(s);
    render();
  })
  .catch((e) => setStatus('error', errorText(e)));
