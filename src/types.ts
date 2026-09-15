/**
 * Shapes shared with the Rust side. Every struct here mirrors one in
 * `src-tauri/crates/soundbench-audio` or `soundbench-dsp` (camelCase via
 * serde). If a field is added there, add it here; nothing else in the app
 * should guess at the JSON.
 */

export type HostKind =
  | 'core-audio'
  | 'asio'
  | 'wasapi'
  | 'wdm-ks'
  | 'direct-sound'
  | 'mme'
  | 'alsa'
  | 'jack'
  | 'pulse-audio'
  | 'other';

export interface HostInfo {
  index: number | null;
  kind: HostKind;
  name: string;
  description: string;
  deviceCount: number;
  defaultInput: number | null;
  defaultOutput: number | null;
  isDefault: boolean;
  available: boolean;
  note?: string;
}

export interface DeviceInfo {
  index: number;
  host: number;
  hostKind: HostKind;
  hostName: string;
  name: string;
  maxInputChannels: number;
  maxOutputChannels: number;
  defaultSampleRate: number;
  defaultLowInputLatencyMs: number;
  defaultLowOutputLatencyMs: number;
  defaultHighInputLatencyMs: number;
  defaultHighOutputLatencyMs: number;
  isDefaultInput: boolean;
  isDefaultOutput: boolean;
}

export interface SampleRateSupport {
  rate: number;
  input: boolean;
  output: boolean;
  duplex: boolean;
}

export interface BufferSizes {
  source: string;
  min: number | null;
  max: number | null;
  preferred: number | null;
  granularity: number | null;
  candidates: number[];
}

export interface KeyValue {
  key: string;
  value: string;
}

export interface DeviceDetail {
  info: DeviceInfo;
  sampleRates: SampleRateSupport[];
  bufferSizes: BufferSizes;
  inputChannelNames: string[];
  outputChannelNames: string[];
  platform: KeyValue[];
  notes: string[];
}

export interface Snapshot {
  portaudio: string;
  platform: string;
  hosts: HostInfo[];
  devices: DeviceInfo[];
  asioBuiltIn: boolean;
}

export interface Target {
  inputDevice: number;
  outputDevice: number;
  inputChannels: number[];
  outputChannels: number[];
  sampleRate: number;
  bufferFrames: number;
  wasapiExclusive: boolean;
  macChangeDevice: boolean;
}

export interface StreamFacts {
  sampleRate: number;
  inputLatencyMs: number;
  outputLatencyMs: number;
  bufferFrames: number;
  hostBufferFrames: number | null;
  hostBufferSource: string | null;
  reportedLoopMs: number | null;
  cpuLoad: number;
}

export interface ChannelReport<T> {
  inputChannel: number;
  analysis: T;
}

// ---- latency

export interface BurstResult {
  playedAt: number;
  delaySamples: number;
  confidence: number;
  found: boolean;
  inverted: boolean;
  levelDbfs: number;
}

export interface LatencyResult {
  bursts: BurstResult[];
  found: number;
  medianSamples: number;
  medianMs: number;
  minSamples: number;
  maxSamples: number;
  spreadSamples: number;
  inverted: boolean;
}

export interface LatencyReport {
  facts: StreamFacts;
  channels: ChannelReport<LatencyResult>[];
  reportedTotalMs: number;
  unreportedMs: number | null;
  measuredMs: number | null;
  measuredSamples: number | null;
}

export interface LatencyOptions {
  bursts: number;
  maxDelayS: number;
  levelDbfs: number;
}

// ---- sweep

export interface SweepAnalysis {
  found: boolean;
  delaySamples: number;
  peakGainDb: number;
  freqs: number[];
  magnitudeDb: number[];
  phaseDeg: number[];
  harmonicsDb: number[][];
  thdPercent: number[];
  thdDb: number[];
  ir: number[];
  irPre: number;
  maxHarmonic: number;
}

export interface SweepReport {
  facts: StreamFacts;
  spec: { f1: number; f2: number; durationS: number; levelDbfs: number };
  channels: ChannelReport<SweepAnalysis>[];
}

export interface SweepOptions {
  f1: number;
  f2: number;
  durationS: number;
  levelDbfs: number;
  maxDelayS: number;
}

// ---- tone

export interface ToneAnalysis {
  found: boolean;
  expectedHz: number;
  fundamentalHz: number;
  frequencyErrorPpm: number;
  levelDbfs: number;
  harmonicsDb: number[];
  thdPercent: number;
  thdDb: number;
  thdNPercent: number;
  thdNDb: number;
  snrDb: number;
  sinadDb: number;
  noiseDbfs: number;
  dcOffset: number;
  fftSize: number;
  bandLoHz: number;
  bandHiHz: number;
  spectrumFreqs: number[];
  spectrumDb: number[];
}

export interface ToneReport {
  facts: StreamFacts;
  frequency: number;
  levelDbfs: number;
  channels: ChannelReport<ToneAnalysis>[];
}

export interface ToneOptions {
  frequency: number;
  durationS: number;
  levelDbfs: number;
}

// ---- noise floor

export interface SpectralPeak {
  hz: number;
  levelDbfs: number;
  prominenceDb: number;
}

export interface NoiseAnalysis {
  rmsDbfs: number;
  bandRmsDbfs: number;
  aWeightedDbfs: number;
  peakDbfs: number;
  dcOffset: number;
  dcOffsetDbfs: number;
  peaks: SpectralPeak[];
  spectrumFreqs: number[];
  spectrumDb: number[];
  fftSize: number;
  blocks: number;
}

export interface NoiseReport {
  facts: StreamFacts;
  channels: ChannelReport<NoiseAnalysis>[];
}

export interface NoiseOptions {
  durationS: number;
}

// ---- transfer (noise)

export interface TransferAnalysis {
  found: boolean;
  delaySamples: number;
  freqs: number[];
  magnitudeDb: number[];
  phaseDeg: number[];
  coherence: number[];
  fftSize: number;
  blocks: number;
}

export interface TransferReport {
  facts: StreamFacts;
  colour: string;
  levelDbfs: number;
  channels: ChannelReport<TransferAnalysis>[];
}

export interface TransferOptions {
  durationS: number;
  levelDbfs: number;
  colour: 'pink' | 'white';
  maxDelayS: number;
}

// ---- stability

export type Verdict = 'clean' | 'xruns-only' | 'glitchy' | 'failed-to-open' | 'no-signal';

export interface GlitchEvent {
  kind: 'discontinuity' | 'dropout';
  atS: number;
  durationMs: number;
  severityDb: number;
}

export interface GlitchReport {
  events: GlitchEvent[];
  discontinuities: number;
  dropouts: number;
  residualMedianDb: number;
  residualMaxDb: number;
  zeroRuns: number;
  amplitudeDbfs: number;
  analysedS: number;
}

export interface CallbackStats {
  callbacks: number;
  inputOverflows: number;
  inputUnderflows: number;
  outputUnderflows: number;
  outputOverflows: number;
  expectedIntervalMs: number;
  meanIntervalMs: number;
  maxIntervalMs: number;
  lateCallbacks: number;
  maxFrames: number;
  minFrames: number;
  cpuLoad: number;
}

export interface StabilityRow {
  requestedFrames: number;
  verdict: Verdict;
  error: string | null;
  facts: StreamFacts | null;
  callbacks: CallbackStats | null;
  latencyMs: number | null;
  latencySamples: number | null;
  glitches: ChannelReport<GlitchReport>[];
}

export interface StabilityReport {
  rows: StabilityRow[];
  durationS: number;
  frequency: number;
}

export interface StabilityOptions {
  bufferSizes: number[];
  durationS: number;
  frequency: number;
  levelDbfs: number;
  maxDelayS: number;
}

/** What the app remembers between runs: the setup, by name rather than index. */
export interface SavedSetup {
  hostKind: HostKind | null;
  outputName: string | null;
  inputName: string | null;
  outputChannels: number[];
  inputChannels: number[];
  sampleRate: number;
  bufferFrames: number;
  wasapiExclusive: boolean;
  macChangeDevice: boolean;
}

export type TestKind = 'latency' | 'sweep' | 'tone' | 'noise' | 'transfer' | 'stability';

export interface ProgressPayload {
  kind: string;
  phase: string;
  fraction: number;
  message: string;
}

export interface Results {
  latency?: LatencyReport;
  sweep?: SweepReport;
  tone?: ToneReport;
  noise?: NoiseReport;
  transfer?: TransferReport;
  stability?: StabilityReport;
}
