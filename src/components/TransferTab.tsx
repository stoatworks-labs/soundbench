/** Transfer function and coherence from pink or white noise. */
import { useMemo } from 'react';

import { fixed, samples } from '../lib/format';
import { COLORS } from '../lib/palette';
import { useStore } from '../store';
import { Plot } from './Plot';
import { Facts, Field, NumberField, RunButton, Stat } from './ui';

export function TransferTab() {
  const r = useStore((s) => s.results.transfer);
  const o = useStore((s) => s.options.transfer);
  const setOptions = useStore((s) => s.setOptions);
  const found = useMemo(() => r?.channels.filter((c) => c.analysis.found) ?? [], [r]);
  const mag = useMemo(() => found.map((c, i) => ({ name: `In ${c.inputChannel + 1}`, x: c.analysis.freqs, y: c.analysis.magnitudeDb, color: COLORS[i % COLORS.length] })), [found]);
  const coh = useMemo(() => found.map((c, i) => ({ name: `In ${c.inputChannel + 1}`, x: c.analysis.freqs, y: c.analysis.coherence, color: COLORS[i % COLORS.length] })), [found]);
  const phase = useMemo(() => found.map((c, i) => ({ name: `In ${c.inputChannel + 1}`, x: c.analysis.freqs, y: c.analysis.phaseDeg, color: COLORS[i % COLORS.length] })), [found]);

  return (
    <div className="tab-body">
      <section className="card">
        <h3>Transfer function from noise</h3>
        <p className="muted">
          The cross-check on the sweep, with a different stimulus and a different estimator: Welch-averaged cross-spectra of the noise played and the noise recorded give the response (H1) and the coherence — 1 where the output is a linear function of the input, lower wherever noise or distortion takes over. Where coherence is high, the sweep and this should agree.
        </p>
        <div className="options">
          <Field label="Noise">
            <select value={o.colour} onChange={(e) => setOptions('transfer', { colour: e.target.value as 'pink' | 'white' })}>
              <option value="pink">Pink (−3 dB/oct)</option>
              <option value="white">White</option>
            </select>
          </Field>
          <NumberField label="Duration" unit="s" value={o.durationS} min={2} max={60} step={1} onChange={(v) => setOptions('transfer', { durationS: v })} />
          <NumberField label="Level" unit="dBFS RMS" value={o.levelDbfs} min={-60} max={-14} step={1} onChange={(v) => setOptions('transfer', { levelDbfs: v })} hint="RMS. Noise peaks 12 dB above its RMS, so the cap is −14 dBFS." />
          <RunButton kind="transfer" />
        </div>
      </section>
      {r ? (
        found.length === 0 ? (
          <p className="note bad">The noise could not be aligned with the recording on any selected input.</p>
        ) : (
          <>
            <div className="stats">
              <Stat label="Aligned at" value={samples(found[0].analysis.delaySamples)} sub="round trip, by phase-transform correlation" />
              <Stat label="Averages" value={`${found[0].analysis.blocks} blocks`} sub={`${found[0].analysis.fftSize}-point, ${r.colour} noise at ${fixed(r.levelDbfs, 0)} dBFS RMS`} />
            </div>
            <Facts facts={r.facts} />
            <section className="card">
              <h3>Magnitude</h3>
              <Plot series={mag} xLog xLabel="Hz" yLabel="dB" yFormat={(v) => v.toFixed(1)} yMarks={[0]} />
            </section>
            <section className="card">
              <h3>Coherence</h3>
              <Plot series={coh} xLog xLabel="Hz" yLabel="γ²" yRange={[0, 1.02]} yFormat={(v) => v.toFixed(2)} height={180} />
            </section>
            <section className="card">
              <h3>Phase</h3>
              <Plot series={phase} xLog xLabel="Hz" yLabel="degrees" yFormat={(v) => `${v.toFixed(0)}°`} yMarks={[0]} height={180} />
            </section>
          </>
        )
      ) : null}
    </div>
  );
}
