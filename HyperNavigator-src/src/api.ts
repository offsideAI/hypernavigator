// Typed wrappers around the Host commands granted to the chrome WebView (capabilities/chrome.json).
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export interface AppInfo {
  id: string;
  name: string;
  version: string;
  origin: string;
  entryUrl: string;
}

export interface NavState {
  url: string | null;
  canGoBack: boolean;
  canGoForward: boolean;
  canReload: boolean;
  status: 'ready' | 'loaded' | 'error';
  message: string | null;
  app: AppInfo | null;
  sharedFolder: { folderName: string; entries: number } | null;
}

export interface SharePrompt {
  requestId: number;
  folderName: string;
  appName: string;
  appOrigin: string;
  maxEntries: number;
}

export type GuestEvent =
  | { kind: 'pageLoad'; session: number; finished: boolean }
  | { kind: 'blocked'; session: number; url: string }
  | { kind: 'leftApp'; session: number; url: string };

export const DEFAULT_MANIFEST_URL = 'http://127.0.0.1:8765/counter/hypernavigator.json';

export const navState = () => invoke<NavState>('nav_state');
export const navigate = (url: string) => invoke<NavState>('navigate', { url });
export const goBack = () => invoke<NavState>('go_back');
export const goForward = () => invoke<NavState>('go_forward');
export const reload = () => invoke<NavState>('reload');
export const pickShareFolder = () => invoke<SharePrompt | null>('pick_share_folder');
export const resolveShareFolder = (requestId: number, approve: boolean) =>
  invoke<NavState>('resolve_share_folder', { requestId, approve });

export const onGuestEvent = (handler: (event: GuestEvent) => void): Promise<UnlistenFn> =>
  listen<GuestEvent>('hn:guest', (event) => handler(event.payload));
