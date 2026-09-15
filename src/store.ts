/**
 * App state.
 *
 * One store: what is plugged in, what the user picked, what each test
 * produced, and whether something is running. Results are kept per test
 * kind and survive re-running another test, so a full report can be
 * assembled from whichever tests have been run on the current setup;
 * changing the input or output device clears them, because a result that
 * belongs to a different interface is worse than no result.
 */
import { create } from 'zustand';

import { api, inTauri } from './lib/ipc';
import type {
  DeviceDetail,
  DeviceInfo,
  HostInfo,
  LatencyOptions,
  NoiseOptions,
  ProgressPayload,
  Results,
  SavedSetup,
  Snapshot,
  StabilityOptions,
  SweepOptions,
  Target,
  TestKind,
  ToneOptions,
  TransferOptions,
} from './types';

export interface Options {
  latency: LatencyOptions;
  sweep: SweepOptions;
  tone: ToneOptions;
  noise: NoiseOptions;
  transfer: TransferOptions;
  stability: StabilityOptions;
}

export const DEFAULT_OPTIONS: Options = {
  latency: { bursts: 5, maxDelayS: 1.0, levelDbfs: -12 },
  sweep: { f1: 10, f2: 22000, durationS: 10, levelDbfs: -12, maxDelayS: 1.0 },
  tone: { frequency: 997, durationS: 4, levelDbfs: -1 },
  noise: { durationS: 4 },
  transfer: { durationS: 8, levelDbfs: -20, colour: 'pink', maxDelayS: 1.0 },
  stability: { bufferSizes: [], durationS: 6, frequency: 1000, levelDbfs: -12, maxDelayS: 1.0 },
};

export const TEST_ORDER: TestKind[] = ['latency', 'sweep', 'tone', 'noise', 'transfer', 'stability'];

export const TEST_NAMES: Record<TestKind, string> = {
  latency: 'Latency',
  sweep: 'Response',
  tone: 'Distortion',
  noise: 'Noise floor',
  transfer: 'Transfer',
  stability: 'Buffer stability',
};

export type Tab = 'device' | TestKind | 'report';

interface State {
  snapshot: Snapshot | null;
  loading: boolean;
  error: string | null;
  tab: Tab;

  hostIndex: number | null;
  inputDevice: number | null;
  outputDevice: number | null;
  inputChannels: number[];
  outputChannels: number[];
  sampleRate: number;
  bufferFrames: number;
  wasapiExclusive: boolean;
  macChangeDevice: boolean;
  inputDetail: DeviceDetail | null;
  outputDetail: DeviceDetail | null;

  options: Options;
  results: Results;
  running: TestKind | null;
  queue: TestKind[];
  progress: ProgressPayload | null;

  restoring: boolean;
  load: () => Promise<void>;
  applySaved: (saved: SavedSetup) => Promise<boolean>;
  saved: () => SavedSetup;
  rescan: () => Promise<void>;
  setTab: (t: Tab) => void;
  setHost: (index: number | null) => void;
  setInputDevice: (index: number | null) => Promise<void>;
  setOutputDevice: (index: number | null) => Promise<void>;
  toggleInputChannel: (ch: number) => void;
  toggleOutputChannel: (ch: number) => void;
  setSampleRate: (r: number) => void;
  setBufferFrames: (b: number) => void;
  setWasapiExclusive: (v: boolean) => void;
  setMacChangeDevice: (v: boolean) => void;
  setOptions: <K extends TestKind>(kind: K, patch: Partial<Options[K]>) => void;
  target: () => Target | null;
  run: (kind: TestKind) => Promise<void>;
  runAll: () => Promise<void>;
  cancel: () => Promise<void>;
  clearResults: () => void;
  setProgress: (p: ProgressPayload | null) => void;
}

function hostsOf(s: Snapshot | null): HostInfo[] {
  return s?.hosts.filter((h) => h.available) ?? [];
}

export function devicesOfHost(s: Snapshot | null, host: number | null): DeviceInfo[] {
  if (!s) return [];
  return s.devices.filter((d) => host === null || d.host === host);
}

function preferredRate(detail: DeviceDetail | null, current: number): number {
  if (!detail) return current;
  const ok = detail.sampleRates.filter((r) => r.duplex || r.input || r.output).map((r) => r.rate);
  if (ok.includes(current)) return current;
  const def = Math.round(detail.info.defaultSampleRate);
  if (ok.includes(def)) return def;
  return ok[0] ?? current;
}

function preferredBuffer(detail: DeviceDetail | null, current: number): number {
  if (!detail) return current;
  const c = detail.bufferSizes.candidates;
  if (c.includes(current)) return current;
  if (detail.bufferSizes.preferred && c.includes(detail.bufferSizes.preferred)) return detail.bufferSizes.preferred;
  return c.includes(256) ? 256 : (c[Math.floor(c.length / 2)] ?? current);
}

export const useStore = create<State>((set, get) => ({
  snapshot: null,
  loading: true,
  error: null,
  tab: 'device',
  restoring: false,

  hostIndex: null,
  inputDevice: null,
  outputDevice: null,
  inputChannels: [0],
  outputChannels: [0],
  sampleRate: 48000,
  bufferFrames: 256,
  wasapiExclusive: true,
  macChangeDevice: true,
  inputDetail: null,
  outputDetail: null,

  options: DEFAULT_OPTIONS,
  results: {},
  running: null,
  queue: [],
  progress: null,

  load: async () => {
    set({ loading: true, error: null });
    try {
      const snapshot = await api.startup();
      set({ snapshot, loading: false });
      const saved = inTauri ? await api.loadSettings().catch(() => null) : null;
      if (saved && (await get().applySaved(saved))) return;
      const hosts = hostsOf(snapshot);
      const def = hosts.find((h) => h.isDefault) ?? hosts[0];
      get().setHost(def?.index ?? null);
    } catch (e) {
      set({ loading: false, error: String(e) });
    }
  },

  applySaved: async (saved) => {
    const snapshot = get().snapshot;
    if (!snapshot) return false;
    const host = snapshot.hosts.find((h) => h.available && h.kind === saved.hostKind);
    if (!host || host.index === null) return false;
    const devs = devicesOfHost(snapshot, host.index);
    const out = devs.find((d) => d.name === saved.outputName && d.maxOutputChannels > 0);
    const inp = devs.find((d) => d.name === saved.inputName && d.maxInputChannels > 0);
    if (!out || !inp) return false;
    set({ hostIndex: host.index, restoring: true });
    await get().setOutputDevice(out.index);
    await get().setInputDevice(inp.index);
    set((s) => ({
      restoring: false,
      outputChannels: saved.outputChannels.filter((c) => c < out.maxOutputChannels).length ? saved.outputChannels.filter((c) => c < out.maxOutputChannels) : s.outputChannels,
      inputChannels: saved.inputChannels.filter((c) => c < inp.maxInputChannels).length ? saved.inputChannels.filter((c) => c < inp.maxInputChannels) : s.inputChannels,
      sampleRate: saved.sampleRate || s.sampleRate,
      bufferFrames: saved.bufferFrames || s.bufferFrames,
      wasapiExclusive: saved.wasapiExclusive ?? s.wasapiExclusive,
      macChangeDevice: saved.macChangeDevice ?? s.macChangeDevice,
    }));
    return true;
  },

  saved: () => {
    const s = get();
    const host = s.snapshot?.hosts.find((h) => h.index === s.hostIndex) ?? null;
    return {
      hostKind: host?.kind ?? null,
      outputName: s.snapshot?.devices.find((d) => d.index === s.outputDevice)?.name ?? null,
      inputName: s.snapshot?.devices.find((d) => d.index === s.inputDevice)?.name ?? null,
      outputChannels: s.outputChannels,
      inputChannels: s.inputChannels,
      sampleRate: s.sampleRate,
      bufferFrames: s.bufferFrames,
      wasapiExclusive: s.wasapiExclusive,
      macChangeDevice: s.macChangeDevice,
    };
  },

  rescan: async () => {
    set({ loading: true, error: null });
    try {
      const before = get().snapshot;
      const { inputDevice, outputDevice } = get();
      const snapshot = await api.rescan();
      set({ snapshot, loading: false, results: {} });
      // Keep the selection if the devices are still there, by name.
      const find = (idx: number | null) => {
        const old = before?.devices.find((d) => d.index === idx);
        return old ? (snapshot.devices.find((d) => d.name === old.name && d.hostKind === old.hostKind)?.index ?? null) : null;
      };
      await get().setInputDevice(find(inputDevice));
      await get().setOutputDevice(find(outputDevice));
    } catch (e) {
      set({ loading: false, error: String(e) });
    }
  },

  setTab: (tab) => set({ tab }),

  setHost: (hostIndex) => {
    const { snapshot } = get();
    set({ hostIndex, results: {} });
    const host = snapshot?.hosts.find((h) => h.index === hostIndex);
    const devs = devicesOfHost(snapshot, hostIndex);
    // The system's own defaults, as every audio app starts; failing that, the
    // first device that does both directions.
    const duplex = devs.find((d) => d.maxInputChannels > 0 && d.maxOutputChannels > 0);
    const output = devs.find((d) => d.index === host?.defaultOutput) ?? duplex ?? devs.find((d) => d.maxOutputChannels > 0);
    const input = devs.find((d) => d.index === host?.defaultInput) ?? duplex ?? devs.find((d) => d.maxInputChannels > 0);
    void get().setInputDevice(input?.index ?? null);
    void get().setOutputDevice(output?.index ?? null);
  },

  setInputDevice: async (index) => {
    set({ inputDevice: index, inputChannels: [0], inputDetail: null, results: {} });
    if (index === null) return;
    try {
      const detail = await api.deviceDetail(index);
      if (get().inputDevice !== index) return;
      set((s) => ({
        inputDetail: detail,
        sampleRate: s.outputDetail ? s.sampleRate : preferredRate(detail, s.sampleRate),
      }));
    } catch (e) {
      set({ error: String(e) });
    }
  },

  setOutputDevice: async (index) => {
    set({ outputDevice: index, outputChannels: [0], outputDetail: null, results: {} });
    if (index === null) return;
    try {
      const detail = await api.deviceDetail(index);
      if (get().outputDevice !== index) return;
      set((s) => ({
        outputDetail: detail,
        sampleRate: preferredRate(detail, s.sampleRate),
        bufferFrames: preferredBuffer(detail, s.bufferFrames),
        options: {
          ...s.options,
          stability: { ...s.options.stability, bufferSizes: detail.bufferSizes.candidates.filter((c) => c >= 16 && c <= 4096) },
        },
      }));
    } catch (e) {
      set({ error: String(e) });
    }
  },

  toggleInputChannel: (ch) =>
    set((s) => {
      const has = s.inputChannels.includes(ch);
      const next = has ? s.inputChannels.filter((c) => c !== ch) : [...s.inputChannels, ch].sort((a, b) => a - b);
      return { inputChannels: next.length ? next : s.inputChannels, results: {} };
    }),
  toggleOutputChannel: (ch) =>
    set((s) => {
      const has = s.outputChannels.includes(ch);
      const next = has ? s.outputChannels.filter((c) => c !== ch) : [...s.outputChannels, ch].sort((a, b) => a - b);
      return { outputChannels: next.length ? next : s.outputChannels, results: {} };
    }),
  setSampleRate: (sampleRate) => set({ sampleRate, results: {} }),
  setBufferFrames: (bufferFrames) => set({ bufferFrames }),
  setWasapiExclusive: (wasapiExclusive) => set({ wasapiExclusive }),
  setMacChangeDevice: (macChangeDevice) => set({ macChangeDevice }),
  setOptions: (kind, patch) => set((s) => ({ options: { ...s.options, [kind]: { ...s.options[kind], ...patch } } })),

  target: () => {
    const s = get();
    if (s.inputDevice === null || s.outputDevice === null) return null;
    return {
      inputDevice: s.inputDevice,
      outputDevice: s.outputDevice,
      inputChannels: s.inputChannels,
      outputChannels: s.outputChannels,
      sampleRate: s.sampleRate,
      bufferFrames: s.bufferFrames,
      wasapiExclusive: s.wasapiExclusive,
      macChangeDevice: s.macChangeDevice,
    };
  },

  run: async (kind) => {
    const target = get().target();
    if (!target) {
      set({ error: 'Pick an input and an output device first.' });
      return;
    }
    if (get().running) return;
    set({ running: kind, error: null, progress: { kind, phase: kind, fraction: 0, message: 'starting' }, tab: kind });
    try {
      const options = get().options[kind];
      const report = await api.runTest<Results[typeof kind]>(kind, target, options);
      set((s) => ({ results: { ...s.results, [kind]: report } }));
    } catch (e) {
      const msg = String(e);
      set({ error: msg === 'cancelled' ? null : msg });
    } finally {
      set({ running: null, progress: null });
    }
  },

  runAll: async () => {
    if (get().running) return;
    set({ queue: [...TEST_ORDER] });
    for (const kind of TEST_ORDER) {
      if (get().queue.length === 0) break;
      set((s) => ({ queue: s.queue.filter((k) => k !== kind) }));
      await get().run(kind);
      if (get().error) break;
    }
    set({ queue: [] });
  },

  cancel: async () => {
    set({ queue: [] });
    await api.cancel();
  },

  clearResults: () => set({ results: {} }),
  setProgress: (progress) => set({ progress }),
}));

// Remember the setup whenever it changes, a moment after it settles. Never
// while a saved setup is still being applied, or the half-applied state
// would overwrite the file it came from.
let saveTimer: ReturnType<typeof setTimeout> | undefined;
useStore.subscribe((s, prev) => {
  if (!inTauri || s.restoring || s.loading) return;
  if (
    s.hostIndex === prev.hostIndex &&
    s.inputDevice === prev.inputDevice &&
    s.outputDevice === prev.outputDevice &&
    s.inputChannels === prev.inputChannels &&
    s.outputChannels === prev.outputChannels &&
    s.sampleRate === prev.sampleRate &&
    s.bufferFrames === prev.bufferFrames &&
    s.wasapiExclusive === prev.wasapiExclusive &&
    s.macChangeDevice === prev.macChangeDevice
  )
    return;
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    void api.saveSettings(useStore.getState().saved()).catch(() => undefined);
  }, 500);
});
