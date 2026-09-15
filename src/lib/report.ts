/**
 * The report: everything measured on the current setup, as Markdown for
 * people and JSON for machines. Pure functions of the store's data.
 */
import type { DeviceDetail, Results, Snapshot, Target } from '../types';
import { db, dbfs, fixed, hz, ms, pct, samples } from './format';

export interface ReportInput {
  app: string;
  version: string;
  when: Date;
  snapshot: Snapshot | null;
  target: Target | null;
  input: DeviceDetail | null;
  output: DeviceDetail | null;
  results: Results;
}

export function reportJson(r: ReportInput): string {
  return JSON.stringify(
    {
      app: r.app,
      version: r.version,
      generated: r.when.toISOString(),
      platform: r.snapshot?.platform,
      portaudio: r.snapshot?.portaudio,
      target: r.target,
      input: r.input,
      output: r.output,
      results: r.results,
    },
    null,
    2,
  );
}

function nearest(freqs: number[], f: number): number {
  let best = 0;
  let bd = Infinity;
  freqs.forEach((v, i) => {
    const d = Math.abs(Math.log(v / f));
    if (d < bd) {
      bd = d;
      best = i;
    }
  });
  return best;
}

export function reportMarkdown(r: ReportInput): string {
  const out: string[] = [];
  const dev = r.output?.info.name ?? '(no device)';
  const same = r.input && r.output && r.input.info.index === r.output.info.index;
  out.push(`# ${r.app} report — ${dev}`);
  out.push('');
  out.push(`Generated ${r.when.toISOString().replace('T', ' ').slice(0, 16)} UTC by ${r.app} ${r.version} on ${r.snapshot?.platform ?? '?'} (${r.snapshot?.portaudio ?? ''}).`);
  out.push('');
  if (r.target) {
    out.push('## Setup');
    out.push('');
    out.push(`- Output: **${r.output?.info.name ?? r.target.outputDevice}** (${r.output?.info.hostName ?? ''}), channel${r.target.outputChannels.length > 1 ? 's' : ''} ${r.target.outputChannels.map((c) => c + 1).join(', ')}`);
    out.push(`- Input: **${r.input?.info.name ?? r.target.inputDevice}** (${r.input?.info.hostName ?? ''}), channel${r.target.inputChannels.length > 1 ? 's' : ''} ${r.target.inputChannels.map((c) => c + 1).join(', ')}`);
    out.push(`- ${r.target.sampleRate} Hz, ${r.target.bufferFrames} frames (${ms((r.target.bufferFrames / r.target.sampleRate) * 1000)})`);
    if (!same) out.push('- Input and output are different devices: separate clocks.');
    out.push('');
  }
  const d = r.output ?? r.input;
  if (d) {
    out.push('## What the driver says');
    out.push('');
    out.push(`- Channels: ${d.info.maxInputChannels} in, ${d.info.maxOutputChannels} out`);
    out.push(`- Sample rates: ${d.sampleRates.filter((s) => s.input || s.output).map((s) => s.rate).join(', ') || 'none probed successfully'}`);
    if (d.bufferSizes.min !== null) out.push(`- Buffer sizes: ${d.bufferSizes.min}–${d.bufferSizes.max} frames${d.bufferSizes.preferred ? `, preferred ${d.bufferSizes.preferred}` : ''} (${d.bufferSizes.source})`);
    out.push(`- Default latency claim: in ${ms(d.info.defaultLowInputLatencyMs)}, out ${ms(d.info.defaultLowOutputLatencyMs)}`);
    for (const kv of d.platform) out.push(`- ${kv.key}: ${kv.value}`);
    out.push('');
  }
  const L = r.results.latency;
  if (L) {
    out.push('## Round-trip latency');
    out.push('');
    if (L.measuredMs !== null) {
      out.push(`- Measured: **${ms(L.measuredMs)}** (${samples(L.measuredSamples)} at ${L.facts.sampleRate} Hz)`);
      out.push(`- Driver reports: ${ms(L.reportedTotalMs)} (in ${ms(L.facts.inputLatencyMs)} + out ${ms(L.facts.outputLatencyMs)})`);
      out.push(`- Unreported: **${ms(L.unreportedMs)}**`);
      const c = L.channels[0].analysis;
      out.push(`- Spread across ${c.found} bursts: ${samples(c.spreadSamples)}; polarity ${c.inverted ? 'inverted' : 'normal'}`);
    } else {
      out.push('- No burst came back.');
    }
    out.push('');
  }
  const S = r.results.sweep;
  if (S) {
    out.push('## Frequency response and distortion (sweep)');
    out.push('');
    out.push(`Sweep ${S.spec.f1} Hz → ${S.spec.f2} Hz, ${S.spec.durationS} s, ${S.spec.levelDbfs} dBFS.`);
    out.push('');
    for (const c of S.channels) {
      const a = c.analysis;
      if (!a.found) {
        out.push(`Input ${c.inputChannel + 1}: not found.`);
        continue;
      }
      out.push(`Input ${c.inputChannel + 1}: loop gain ${db(a.peakGainDb, 2)}, impulse peak at ${samples(a.delaySamples)}.`);
      out.push('');
      out.push('| Frequency | Level | Phase | THD | H2 | H3 |');
      out.push('| --- | --- | --- | --- | --- | --- |');
      for (const f of [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 15000, 20000]) {
        if (f > S.facts.sampleRate * 0.45) continue;
        const i = nearest(a.freqs, f);
        out.push(`| ${hz(f)} | ${db(a.magnitudeDb[i], 2)} | ${fixed(a.phaseDeg[i], 1)}° | ${pct(a.thdPercent[i])} | ${db(a.harmonicsDb[0]?.[i])} | ${db(a.harmonicsDb[1]?.[i])} |`);
      }
      out.push('');
    }
  }
  const T = r.results.tone;
  if (T) {
    out.push(`## Distortion and noise (${hz(T.frequency)} tone at ${T.levelDbfs} dBFS)`);
    out.push('');
    for (const c of T.channels) {
      const a = c.analysis;
      if (!a.found) {
        out.push(`- Input ${c.inputChannel + 1}: no tone found.`);
        continue;
      }
      out.push(`- Input ${c.inputChannel + 1}: THD+N **${pct(a.thdNPercent)}** (${db(a.thdNDb)}), THD ${pct(a.thdPercent)} (${db(a.thdDb)}), SNR ${db(a.snrDb)}, SINAD ${db(a.sinadDb)}, level ${dbfs(a.levelDbfs, 2)}, frequency ${fixed(a.fundamentalHz, 3)} Hz (${fixed(a.frequencyErrorPpm, 1)} ppm), harmonics ${a.harmonicsDb.map((h, i) => `H${i + 2} ${db(h)}`).join(', ')}`);
    }
    out.push('');
  }
  const N = r.results.noise;
  if (N) {
    out.push('## Noise floor');
    out.push('');
    for (const c of N.channels) {
      const a = c.analysis;
      out.push(`- Input ${c.inputChannel + 1}: **${dbfs(a.bandRmsDbfs)}** (20 Hz–20 kHz), ${dbfs(a.aWeightedDbfs)}(A), peak ${dbfs(a.peakDbfs)}, DC ${fixed(a.dcOffset * 100, 4)} % FS${a.peaks.length ? `; peaks: ${a.peaks.map((p) => `${hz(p.hz)} at ${dbfs(p.levelDbfs)}`).join(', ')}` : ''}`);
    }
    out.push('');
  }
  const X = r.results.transfer;
  if (X) {
    out.push(`## Transfer function (${X.colour} noise)`);
    out.push('');
    for (const c of X.channels) {
      const a = c.analysis;
      if (!a.found) {
        out.push(`- Input ${c.inputChannel + 1}: could not align.`);
        continue;
      }
      const cohLow = a.freqs.map((f, i) => (f > 20 && f < 20000 && a.coherence[i] < 0.95 ? f : NaN)).filter(Number.isFinite);
      out.push(`- Input ${c.inputChannel + 1}: aligned at ${samples(a.delaySamples)}, ${a.blocks} averages; coherence ≥ 0.95 ${cohLow.length ? `except around ${cohLow.slice(0, 5).map((f) => hz(f)).join(', ')}${cohLow.length > 5 ? '…' : ''}` : 'everywhere from 20 Hz to 20 kHz'}`);
    }
    out.push('');
  }
  const B = r.results.stability;
  if (B) {
    out.push('## Buffer stability');
    out.push('');
    out.push('| Requested | Host used | Verdict | Round trip | Driver claims | Xruns | Glitches | Late callbacks |');
    out.push('| --- | --- | --- | --- | --- | --- | --- | --- |');
    for (const row of B.rows) {
      const c = row.callbacks;
      const g = row.glitches[0]?.analysis;
      const x = c ? c.inputOverflows + c.inputUnderflows + c.outputUnderflows + c.outputOverflows : 0;
      out.push(
        `| ${row.requestedFrames} | ${row.facts?.hostBufferFrames ?? '—'} | ${row.verdict}${row.error ? ` (${row.error})` : ''} | ${ms(row.latencyMs)} | ${row.facts ? ms(row.facts.inputLatencyMs + row.facts.outputLatencyMs) : '—'} | ${c ? x : '—'} | ${g ? g.discontinuities + g.dropouts : '—'} | ${c ? c.lateCallbacks : '—'} |`,
      );
    }
    out.push('');
  }
  return out.join('\n');
}
