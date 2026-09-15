/** Noise floor of the input with silence at the output. */
import { useMemo } from 'react';

import { db, dbfs, fixed, hz } from '../lib/format';
import { COLORS } from '../lib/palette';
import { useStore } from '../store';
import { Plot } from './Plot';
import { Facts, NumberField, RunButton, Stat } from './ui';

export function NoiseTab() {
  const r = useStore((s) => s.results.noise);
  const o = useStore((s) => s.options.noise);
  const setOptions = useStore((s) => s.setOptions);
  const names = useStore((s) => s.inputDetail?.inputChannelNames ?? []);
  const spectrum = useMemo(() => (r?.channels ?? []).map((c, i) => ({ name: `In ${c.inputChannel + 1}`, x: c.analysis.spectrumFreqs, y: c.analysis.spectrumDb, color: COLORS[i % COLORS.length], width: 1.2 })), [r]);

  return (
    <div className="tab-body">
      <section className="card">
        <h3>Noise floor</h3>
        <p className="muted">
          Digital silence goes out; the input is recorded with the stream running. Reported unweighted in the 20 Hz–20 kHz band (subtract from 0 dBFS for the dynamic range a spec sheet quotes), A-weighted, and as a peak. Narrow peaks standing above the floor are listed: hum, USB packet whine and a switch-mode supply each have their own frequency.
        </p>
        <div className="options">
          <NumberField label="Duration" unit="s" value={o.durationS} min={1} max={60} step={1} onChange={(v) => setOptions('noise', { durationS: v })} />
          <RunButton kind="noise" />
        </div>
      </section>
      {r ? (
        <>
          {r.channels.map((c) => {
            const a = c.analysis;
            return (
              <div key={c.inputChannel}>
                <h3 className="sub">
                  Input {c.inputChannel + 1}
                  {names[c.inputChannel] ? ` — ${names[c.inputChannel]}` : ''}
                </h3>
                <div className="stats">
                  <Stat label="Noise, 20 Hz – 20 kHz" value={dbfs(a.bandRmsDbfs)} sub={`dynamic range ${db(-a.bandRmsDbfs)}`} tone={a.bandRmsDbfs < -100 ? 'good' : a.bandRmsDbfs < -85 ? 'warn' : 'bad'} />
                  <Stat label="A-weighted" value={`${dbfs(a.aWeightedDbfs)}(A)`} />
                  <Stat label="Full band" value={dbfs(a.rmsDbfs)} sub="to Nyquist" />
                  <Stat label="Peak" value={dbfs(a.peakDbfs)} />
                  <Stat label="DC offset" value={fixed(a.dcOffset * 100, 4, '% FS')} sub={dbfs(a.dcOffsetDbfs)} />
                </div>
                {a.peaks.length ? (
                  <table className="grid compact">
                    <thead>
                      <tr>
                        <th>Peak</th>
                        <th>Level</th>
                        <th>Above floor</th>
                      </tr>
                    </thead>
                    <tbody>
                      {a.peaks.map((p) => (
                        <tr key={p.hz}>
                          <td>{hz(p.hz)}</td>
                          <td>{dbfs(p.levelDbfs)}</td>
                          <td>{db(p.prominenceDb)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                ) : (
                  <p className="muted small">No narrow peaks stand more than 12 dB above the surrounding floor.</p>
                )}
              </div>
            );
          })}
          <Facts facts={r.facts} />
          <section className="card">
            <h3>Spectrum</h3>
            <p className="muted small">dBFS per bin, {r.channels[0]?.analysis.fftSize}-point blocks, {r.channels[0]?.analysis.blocks} averaged.</p>
            <Plot series={spectrum} xLog xLabel="Hz" yLabel="dBFS" yRange={[-180, -40]} yFormat={(v) => `${v.toFixed(0)}`} height={300} />
          </section>
        </>
      ) : null}
    </div>
  );
}
