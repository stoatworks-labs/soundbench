/** THD+N, SNR and level from a single tone. */
import { useMemo } from 'react';

import { db, dbfs, fixed, hz, pct } from '../lib/format';
import { COLORS } from '../lib/palette';
import { useStore } from '../store';
import { Plot } from './Plot';
import { Facts, NumberField, RunButton, Stat } from './ui';

export function ToneTab() {
  const r = useStore((s) => s.results.tone);
  const o = useStore((s) => s.options.tone);
  const setOptions = useStore((s) => s.setOptions);
  const names = useStore((s) => s.inputDetail?.inputChannelNames ?? []);
  const found = useMemo(() => r?.channels.filter((c) => c.analysis.found) ?? [], [r]);
  const spectrum = useMemo(() => found.map((c, i) => ({ name: `In ${c.inputChannel + 1}`, x: c.analysis.spectrumFreqs, y: c.analysis.spectrumDb, color: COLORS[i % COLORS.length], width: 1.2 })), [found]);

  return (
    <div className="tab-body">
      <section className="card">
        <h3>Distortion and noise from a tone</h3>
        <p className="muted">
          One steady sine, one long FFT under a seven-term window. THD is the harmonics against the fundamental; THD+N is everything else in the 20 Hz–20 kHz band against it, the way AES17 measures it. 997 Hz rather than 1 kHz so the harmonics do not sit on FFT bin boundaries. The measured frequency against the generated one reads the clock difference between the converters.
        </p>
        <div className="options">
          <NumberField label="Frequency" unit="Hz" value={o.frequency} min={10} max={20000} step={1} onChange={(v) => setOptions('tone', { frequency: v })} />
          <NumberField label="Duration" unit="s" value={o.durationS} min={1} max={60} step={1} onChange={(v) => setOptions('tone', { durationS: v })} />
          <NumberField label="Level" unit="dBFS" value={o.levelDbfs} min={-60} max={0} step={1} onChange={(v) => setOptions('tone', { levelDbfs: v })} hint="−1 dBFS is the AES17 THD+N level. Back it off if the interface's input clips." />
          <RunButton kind="tone" />
        </div>
      </section>
      {r ? (
        found.length === 0 ? (
          <p className="note bad">No tone came back above −90 dBFS on any selected input.</p>
        ) : (
          <>
            {found.map((c) => {
              const a = c.analysis;
              return (
                <div key={c.inputChannel}>
                  <h3 className="sub">
                    Input {c.inputChannel + 1}
                    {names[c.inputChannel] ? ` — ${names[c.inputChannel]}` : ''}
                  </h3>
                  <div className="stats">
                    <Stat label="THD+N" value={pct(a.thdNPercent)} sub={db(a.thdNDb)} tone={a.thdNDb < -80 ? 'good' : a.thdNDb < -60 ? 'warn' : 'bad'} />
                    <Stat label="THD" value={pct(a.thdPercent)} sub={db(a.thdDb)} />
                    <Stat label="SNR" value={db(a.snrDb)} sub={`noise ${dbfs(a.noiseDbfs)} in band`} />
                    <Stat label="SINAD" value={db(a.sinadDb)} />
                    <Stat label="Level received" value={dbfs(a.levelDbfs, 2)} sub={`sent ${fixed(r.levelDbfs, 1)} dBFS → loop gain ${db(a.levelDbfs - r.levelDbfs, 2)}`} />
                    <Stat label="Frequency" value={hz(a.fundamentalHz)} sub={`${fixed(a.frequencyErrorPpm, 1)} ppm from ${hz(a.expectedHz)}`} tone={Math.abs(a.frequencyErrorPpm) < 5 ? 'good' : Math.abs(a.frequencyErrorPpm) < 50 ? 'warn' : 'bad'} />
                    <Stat label="DC offset" value={fixed(a.dcOffset * 100, 4, '% FS')} />
                  </div>
                  <table className="grid compact">
                    <thead>
                      <tr>
                        {a.harmonicsDb.map((_, i) => (
                          <th key={i}>H{i + 2}</th>
                        ))}
                      </tr>
                    </thead>
                    <tbody>
                      <tr>
                        {a.harmonicsDb.map((h, i) => (
                          <td key={i}>{db(h)}</td>
                        ))}
                      </tr>
                    </tbody>
                  </table>
                </div>
              );
            })}
            <Facts facts={r.facts} />
            <section className="card">
              <h3>Spectrum</h3>
              <p className="muted small">dBFS per bin, {found[0]?.analysis.fftSize}-point FFT; max-held onto a log grid.</p>
              <Plot series={spectrum} xLog xLabel="Hz" yLabel="dBFS" yRange={[-160, 0]} yFormat={(v) => `${v.toFixed(0)}`} height={300} />
            </section>
          </>
        )
      ) : null}
    </div>
  );
}
