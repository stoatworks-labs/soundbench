import { describe, expect, it } from 'vitest';

import { reportJson, reportMarkdown, type ReportInput } from './report';
import type { DeviceDetail, StreamFacts } from '../types';

const facts: StreamFacts = {
  sampleRate: 48000,
  inputLatencyMs: 2.67,
  outputLatencyMs: 5.33,
  bufferFrames: 128,
  hostBufferFrames: 128,
  hostBufferSource: 'CoreAudio device buffer frame size',
  reportedLoopMs: 8,
  cpuLoad: 0.001,
};

const detail: DeviceDetail = {
  info: {
    index: 8,
    host: 0,
    hostKind: 'core-audio',
    hostName: 'Core Audio',
    name: 'Test Bridge',
    maxInputChannels: 2,
    maxOutputChannels: 2,
    defaultSampleRate: 48000,
    defaultLowInputLatencyMs: 10,
    defaultLowOutputLatencyMs: 1.3,
    defaultHighInputLatencyMs: 100,
    defaultHighOutputLatencyMs: 10,
    isDefaultInput: false,
    isDefaultOutput: false,
  },
  sampleRates: [{ rate: 48000, input: true, output: true, duplex: true }],
  bufferSizes: { source: 'test', min: 16, max: 4096, preferred: null, granularity: null, candidates: [16, 128] },
  inputChannelNames: ['In 1', 'In 2'],
  outputChannelNames: ['Out 1', 'Out 2'],
  platform: [{ key: 'Transport', value: 'Virtual' }],
  notes: [],
};

const input: ReportInput = {
  app: 'SoundBench',
  version: 'v0.1.0',
  when: new Date('2026-09-15T10:00:00Z'),
  snapshot: { portaudio: 'PortAudio V19.7.0-devel', platform: 'macOS aarch64', hosts: [], devices: [detail.info], asioBuiltIn: false },
  target: { inputDevice: 8, outputDevice: 8, inputChannels: [0], outputChannels: [0], sampleRate: 48000, bufferFrames: 128, wasapiExclusive: false, macChangeDevice: true },
  input: detail,
  output: detail,
  results: {
    latency: {
      facts,
      channels: [{ inputChannel: 0, analysis: { bursts: [], found: 5, medianSamples: 384, medianMs: 8, minSamples: 384, maxSamples: 384, spreadSamples: 0, inverted: false } }],
      reportedTotalMs: 8,
      unreportedMs: 0,
      measuredMs: 8,
      measuredSamples: 384,
    },
    stability: {
      durationS: 6,
      frequency: 1000,
      rows: [
        {
          requestedFrames: 128,
          verdict: 'clean',
          error: null,
          facts,
          callbacks: { callbacks: 100, inputOverflows: 0, inputUnderflows: 0, outputUnderflows: 0, outputOverflows: 0, expectedIntervalMs: 2.67, meanIntervalMs: 2.67, maxIntervalMs: 3, lateCallbacks: 0, maxFrames: 128, minFrames: 128, cpuLoad: 0.001 },
          latencyMs: 8,
          latencySamples: 384,
          glitches: [{ inputChannel: 0, analysis: { events: [], discontinuities: 0, dropouts: 0, residualMedianDb: -150, residualMaxDb: -140, zeroRuns: 0, amplitudeDbfs: -12, analysedS: 5.6 } }],
        },
      ],
    },
  },
};

describe('report', () => {
  it('writes the setup, the driver facts and every measured section', () => {
    const md = reportMarkdown(input);
    expect(md).toContain('# SoundBench report — Test Bridge');
    expect(md).toContain('Output: **Test Bridge**');
    expect(md).toContain('Transport: Virtual');
    expect(md).toContain('Measured: **8.00 ms** (384 samples at 48000 Hz)');
    expect(md).toContain('Unreported: **0.00 ms**');
    expect(md).toContain('| 128 | 128 | clean | 8.00 ms | 8.00 ms | 0 | 0 | 0 |');
    // Sections that were not run are not mentioned.
    expect(md).not.toContain('Frequency response and distortion');
    expect(md).not.toContain('Noise floor');
  });

  it('json carries the results verbatim', () => {
    const j = JSON.parse(reportJson(input));
    expect(j.results.latency.measuredSamples).toBe(384);
    expect(j.generated).toBe('2026-09-15T10:00:00.000Z');
    expect(j.output.info.name).toBe('Test Bridge');
  });
});
