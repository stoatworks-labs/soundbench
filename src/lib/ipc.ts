/**
 * Typed wrappers over the Tauri commands.
 *
 * Nothing else in the app calls `invoke` or `listen` directly, so every
 * command name is in one file and a rename on the Rust side breaks exactly
 * one place.
 */
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import type { DeviceDetail, ProgressPayload, SavedSetup, Snapshot, Target, TestKind } from '../types';

export const api = {
  startup: () => invoke<Snapshot>('startup'),
  rescan: () => invoke<Snapshot>('rescan'),
  deviceDetail: (index: number) => invoke<DeviceDetail>('device_detail', { index }),
  runTest: <T>(kind: TestKind, target: Target, options: object) =>
    invoke<T>('run_test', { kind, target, options }),
  cancel: () => invoke<void>('cancel_test'),
  isRunning: () => invoke<boolean>('is_running'),
  saveText: (path: string, text: string) => invoke<void>('save_text', { path, text }),
  showAsioPanel: (device: number) => invoke<void>('show_asio_panel', { device }),
  loadSettings: () => invoke<SavedSetup | null>('load_settings'),
  saveSettings: (settings: SavedSetup) => invoke<void>('save_settings', { settings }),
  onProgress: (handler: (p: ProgressPayload) => void): Promise<UnlistenFn> =>
    listen<ProgressPayload>('soundbench://progress', (e) => handler(e.payload)),
};

/** True inside the Tauri webview; false in a plain browser tab. */
export const inTauri = '__TAURI_INTERNALS__' in window;
