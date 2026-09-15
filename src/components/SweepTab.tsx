/** Frequency response, phase and harmonic distortion from the log sweep. */
import { useMemo } from 'react';

import { db, fixed, pct, samples } from '../lib/format';
import { COLORS } from '../lib/palette';
import { useStore } from '../store';
import type { SweepAnalysis } from '../types';
import { Plot } from './Plot';
import { Facts, NumberField, RunButton, Stat } from './ui';

function at(a: SweepAnalysis, f: number): number {
  let best = 0;
  let bd = Infinity;
  a.freqs.forEach((v, i) => {
    const d = Math.abs(Math.log(v / f));
    if (d < bd) {
      bd = d;
      best = i;
    }
  });
  return best;
}

/** Ripple within a band: max − min of the magnitude curve. */
function ripple(a: SweepAnalysis, lo: number, hi: number): number {
  const vals = a.freqs.map((f, i) => (f >= lo && f <= hi ? a.magnitudeDb[i] : NaN)).filter(Number.isFinite);
  if (!vals.length) return NaN;
  return Math.max(...vals) - Math.min(...vals);
}

/** Where the response has fallen 3 dB below its 1 kHz level, at each end. */
function corners(a: SweepAnalysis): [number | null, number | null] {
  const ref = a.magnitudeDb[at(a, 1000)];
  let lo: number | null = null;
  let hi: number | null = null;
  for (let i = at(a, 1000); i >= 0; i--) {
    if (a.magnitudeDb[i] < ref - 3) {
      lo = a.freqs[i];
      break;
    }
  }
  for (let i = at(a, 1000); i < a.freqs.length; i++) {
    if (a.magnitudeDb[i] < ref - 3) {
      hi = a.freqs[i];
      break;
    }
  }
  return [lo, hi];
}

export function SweepTab() {
  const r = useStore((s) => s.results.sweep);
  const o = useStore((s) => s.options.sweep);
  const setOptions = useStore((s) => s.setOptions);
  const names = useStore((s) => s.inputDetail?.inputChannelNames ?? []);
  const rate = useStore((s) => s.sampleRate);

  const found = useMemo(() => r?.channels.filter((c) => c.analysis.found) ?? [], [r]);
  const magSeries = useMemo(() => found.map((c, i) => ({ name: `In ${c.inputChannel + 1}`, x: c.analysis.freqs, y: c.analysis.magnitudeDb, color: COLORS[i % COLORS.length] })), [found]);
  const phaseSeries = useMemo(() => found.map((c, i) => ({ name: `In ${c.inputChannel + 1}`, x: c.analysis.freqs, y: c.analysis.phaseDeg, color: COLORS[i % COLORS.length] })), [found]);
  const thdSeries = useMemo(
    () =>
      found.flatMap((c, i) => [
        { name: `THD in ${c.inputChannel + 1}`, x: c.analysis.freqs, y: c.analysis.thdDb, color: COLORS[i % COLORS.length], width: 2 },
        ...(c.analysis.harmonicsDb[0] ? [{ name: `H2`, x: c.analysis.freqs, y: c.analysis.harmonicsDb[0], color: COLORS[(i + 1) % COLORS.length], dashed: true }] : []),
        ...(c.analysis.harmonicsDb[1] ? [{ name: `H3`, x: c.analysis.freqs, y: c.analysis.harmonicsDb[1], color: COLORS[(i + 2) % COLORS.length], dashed: true }] : []),
      ]),
    [found],
  );
  const irSeries = useMemo(
    () =>
      found.map((c, i) => ({
        name: `In ${c.inputChannel + 1}`,
        x: c.analysis.ir.map((_, k) => ((k - c.analysis.irPre) / (r?.facts.sampleRate ?? rate)) * 1000),
        y: c.analysis.ir,
        color: COLORS[i % COLORS.length],
      })),
    [found, r, rate],
  );
  const magRange = useMemo<[number, number]>(() => {
    const ys = magSeries.flatMap((s) => s.y.filter(Number.isFinite));
    if (!ys.length) return [-24, 6];
    const hi = Math.max(...ys);
    const lo = Math.min(...ys);
    return [Math.min(lo - 1, hi - 6), Math.max(hi + 1, lo + 6)];
  }, [magSeries]);

  return (
    <div className="tab-body">
      <section className="card">
        <h3>Frequency response, phase and harmonic distortion</h3>
        <p className="muted">
          An exponential sine sweep, deconvolved with its own inverse filter. The linear part of the loop becomes an impulse response; each order of harmonic distortion becomes its own smaller impulse response, arriving a fixed time before it, so one sweep gives the response, the phase and THD against frequency. Everything is relative to the sweep through a perfect wire.
        </p>
        <div className="options">
          <NumberField label="From" unit="Hz" value={o.f1} min={5} max={1000} step={1} onChange={(v) => setOptions('sweep', { f1: v })} />
          <NumberField label="To" unit="Hz" value={o.f2} min={1000} max={96000} step={100} onChange={(v) => setOptions('sweep', { f2: v })} hint="Capped just under Nyquist." />
          <NumberField label="Duration" unit="s" value={o.durationS} min={1} max={60} step={1} onChange={(v) => setOptions('sweep', { durationS: v })} hint="Longer sweeps separate the harmonics better and average more noise out." />
          <NumberField label="Level" unit="dBFS" value={o.levelDbfs} min={-60} max={0} step={1} onChange={(v) => setOptions('sweep', { levelDbfs: v })} />
          <RunButton kind="sweep" />
        </div>
      </section>

      {r ? (
        found.length === 0 ? (
          <p className="note bad">The sweep did not come back on any selected input. Check the loopback and the levels.</p>
        ) : (
          <>
            <div className="stats">
              {found.slice(0, 1).map((c) => {
                const a = c.analysis;
                const [lo, hi] = corners(a);
                const i1k = at(a, 1000);
                return (
                  <>
                    <Stat key="gain" label="Loop gain" value={db(a.peakGainDb, 2)} sub={`at 1 kHz: ${db(a.magnitudeDb[i1k], 2)}`} />
                    <Stat key="ripple" label="Ripple 20 Hz – 20 kHz" value={db(ripple(a, 20, Math.min(20000, rate * 0.45)), 2)} sub="max − min of the magnitude" tone={ripple(a, 20, Math.min(20000, rate * 0.45)) < 0.5 ? 'good' : 'warn'} />
                    <Stat key="corners" label="−3 dB corners" value={`${lo ? `${fixed(lo, lo < 100 ? 1 : 0)} Hz` : '< start'} · ${hi ? (hi >= 1000 ? `${fixed(hi / 1000, 2)} kHz` : `${fixed(hi, 0)} Hz`) : '> end'}`} sub="relative to 1 kHz" />
                    <Stat key="thd" label="THD at 1 kHz" value={pct(a.thdPercent[i1k])} sub={db(a.thdDb[i1k])} />
                    <Stat key="delay" label="Impulse response peak" value={samples(a.delaySamples)} sub="round trip, from the sweep" />
                  </>
                );
              })}
            </div>
            <Facts facts={r.facts} />
            <section className="card">
              <h3>Magnitude</h3>
              <Plot series={magSeries} xLog xLabel="Hz" yLabel="dB" yRange={magRange} yFormat={(v) => `${v.toFixed(1)}`} yMarks={[0]} />
            </section>
            <section className="card">
              <h3>Phase (round-trip delay removed)</h3>
              <Plot series={phaseSeries} xLog xLabel="Hz" yLabel="degrees" yFormat={(v) => `${v.toFixed(0)}°`} yMarks={[0]} height={200} />
            </section>
            <section className="card">
              <h3>Harmonic distortion against frequency</h3>
              <p className="muted small">THD (solid) and the second and third harmonics (dashed), relative to the fundamental at each frequency. Blank above the frequency whose harmonic would exceed the sweep's top, and in the first octave above the sweep's start.</p>
              <Plot series={thdSeries} xLog xLabel="Hz (fundamental)" yLabel="dB re fundamental" yRange={[-140, 0]} yFormat={(v) => `${v.toFixed(0)}`} />
            </section>
            <section className="card">
              <h3>Impulse response</h3>
              <Plot series={irSeries} xLabel="ms from peak" yLabel="normalised" xFormat={(v) => `${v.toFixed(1)}`} yFormat={(v) => v.toFixed(2)} height={180} yMarks={[0]} />
            </section>
            {found.map((c) => (
              <section className="card" key={c.inputChannel}>
                <h3>
                  Input {c.inputChannel + 1}
                  {names[c.inputChannel] ? ` — ${names[c.inputChannel]}` : ''}: readings
                </h3>
                <table className="grid">
                  <thead>
                    <tr>
                      <th>Frequency</th>
                      <th>Level</th>
                      <th>Phase</th>
                      <th>THD</th>
                      <th>H2</th>
                      <th>H3</th>
                    </tr>
                  </thead>
                  <tbody>
                    {[20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 15000, 20000].filter((f) => f <= rate * 0.45).map((f) => {
                      const i = at(c.analysis, f);
                      return (
                        <tr key={f}>
                          <td>{f >= 1000 ? `${f / 1000} kHz` : `${f} Hz`}</td>
                          <td>{db(c.analysis.magnitudeDb[i], 2)}</td>
                          <td>{fixed(c.analysis.phaseDeg[i], 1)}°</td>
                          <td>{pct(c.analysis.thdPercent[i])}</td>
                          <td>{db(c.analysis.harmonicsDb[0]?.[i])}</td>
                          <td>{db(c.analysis.harmonicsDb[1]?.[i])}</td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </section>
            ))}
          </>
        )
      ) : null}
    </div>
  );
}
