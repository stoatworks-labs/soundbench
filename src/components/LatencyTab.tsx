/** Round-trip latency: the number a DAW has to compensate, and whether the driver admits to it. */
import { dbfs, fixed, ms, samples } from '../lib/format';
import { useStore } from '../store';
import { Facts, NumberField, RunButton, Stat } from './ui';

export function LatencyTab() {
  const r = useStore((s) => s.results.latency);
  const o = useStore((s) => s.options.latency);
  const setOptions = useStore((s) => s.setOptions);
  const rate = useStore((s) => s.sampleRate);
  const names = useStore((s) => s.inputDetail?.inputChannelNames ?? []);

  return (
    <div className="tab-body">
      <section className="card">
        <h3>Round-trip latency</h3>
        <p className="muted">
          Plays a short chirp {o.bursts} times and finds each one in the recording by cross-correlation. The delay from where it was written to where it came back is the whole loop — output buffers, DAC, cable, ADC, input buffers — in samples of the stream's own clock, so there is nothing to calibrate.
        </p>
        <div className="options">
          <NumberField label="Bursts" value={o.bursts} min={1} max={50} step={1} onChange={(v) => setOptions('latency', { bursts: Math.round(v) })} />
          <NumberField label="Listen up to" unit="s" value={o.maxDelayS} min={0.1} max={10} step={0.1} onChange={(v) => setOptions('latency', { maxDelayS: v })} hint="Raise this for network or virtual devices with long buffers." />
          <NumberField label="Level" unit="dBFS" value={o.levelDbfs} min={-60} max={0} step={1} onChange={(v) => setOptions('latency', { levelDbfs: v })} />
          <RunButton kind="latency" />
        </div>
      </section>

      {r ? (
        <>
          <div className="stats">
            <Stat
              label="Measured round trip"
              value={r.measuredMs !== null ? ms(r.measuredMs) : 'not found'}
              sub={r.measuredSamples !== null ? `${samples(r.measuredSamples)} at ${r.facts.sampleRate} Hz` : 'No burst came back. Check the loopback cable and the channel selection.'}
              tone={r.measuredMs !== null ? 'good' : 'bad'}
            />
            <Stat label="Driver reports" value={ms(r.reportedTotalMs)} sub={`in ${ms(r.facts.inputLatencyMs)} + out ${ms(r.facts.outputLatencyMs)}`} />
            <Stat
              label="Unreported"
              value={r.unreportedMs !== null ? ms(r.unreportedMs) : '—'}
              sub={r.unreportedMs !== null ? (Math.abs(r.unreportedMs) < 0.3 ? 'the driver tells the truth' : r.unreportedMs > 0 ? 'latency the driver does not declare — a DAW compensating from the driver will be early by this' : 'the driver over-declares; compensation would be late') : ''}
              tone={r.unreportedMs === null ? undefined : Math.abs(r.unreportedMs) < 0.3 ? 'good' : Math.abs(r.unreportedMs) < 2 ? 'warn' : 'bad'}
            />
            <Stat
              label="Stability"
              value={r.channels[0] ? samples(r.channels[0].analysis.spreadSamples) : '—'}
              sub="spread between bursts; more than a sample means the loop's delay moves"
              tone={r.channels[0] ? (r.channels[0].analysis.spreadSamples <= 1 ? 'good' : 'warn') : undefined}
            />
            <Stat label="Polarity" value={r.channels[0]?.analysis.found ? (r.channels[0].analysis.inverted ? 'inverted' : 'normal') : '—'} tone={r.channels[0]?.analysis.inverted ? 'warn' : undefined} />
          </div>
          <Facts facts={r.facts} />
          {r.channels.map((c) => (
            <section className="card" key={c.inputChannel}>
              <h3>
                Input {c.inputChannel + 1}
                {names[c.inputChannel] ? ` — ${names[c.inputChannel]}` : ''}
              </h3>
              <table className="grid">
                <thead>
                  <tr>
                    <th>Burst</th>
                    <th>Played at</th>
                    <th>Delay</th>
                    <th>ms</th>
                    <th>Level</th>
                    <th>Confidence</th>
                    <th>Polarity</th>
                  </tr>
                </thead>
                <tbody>
                  {c.analysis.bursts.map((b, i) => (
                    <tr key={i} className={b.found ? '' : 'dim'}>
                      <td>{i + 1}</td>
                      <td>{fixed(b.playedAt / rate, 3)} s</td>
                      <td>{b.found ? samples(b.delaySamples) : 'not found'}</td>
                      <td>{b.found ? ms((b.delaySamples / rate) * 1000) : '—'}</td>
                      <td>{b.found ? dbfs(b.levelDbfs) : '—'}</td>
                      <td>{fixed(b.confidence, 0)}</td>
                      <td>{b.found ? (b.inverted ? 'inverted' : 'normal') : '—'}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          ))}
        </>
      ) : null}
    </div>
  );
}
