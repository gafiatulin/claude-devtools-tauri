/**
 * Tauri API adapter.
 *
 * Uses native Tauri invoke() calls — no HTTP server, no Node.js process.
 * The Rust backend is always available in-process.
 */

import { TauriClient } from './tauriClient';

import type { ElectronAPI } from '@shared/types/api';

export const api: ElectronAPI = new TauriClient();
