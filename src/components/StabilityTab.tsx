/** Which buffer sizes the interface runs cleanly at. */
import { db, fixed, ms } from '../lib/format';
import { useStore } from '../store';
import type { StabilityRow, Verdict } from '../types';
import { Facts, NumberField, RunButton, Stat } from './ui';

const VERDICT: Record<Verdict, { label: string; tone: 'good' | 'warn' | 'bad' }> = {
  clean: { label: 'Clean', tone: 'good' },
  'xruns-only': { label: 'Xruns, audio intact', tone: 'warn' },
  glitchy: { label: 'Glitches', tone: 'bad' },
  'failed-to-open': { label: 'Would not open', tone: 'bad' },
  'no-signal': { label: 'No signal back', tone: 'bad' },
};

function xruns(row: StabilityRow): string {
  const c = row.callbacks;
  if (!c) return '—';
  const n = c.inputOverflows + c.inputUnderflows + c.outputUnderflows + c.outputOverflows;
  if (n === 0) return 'none';
  const parts = [];
  if (c.outputUnderflows) parts.push(`${c.outputUnderflows} out under`);
  if (c.inputOverflows) parts.push(`${c.inputOverflows} in over`);
  if (c.inputUnderflows) parts.push(`${c.inputUnderflows} in under`);
  if (c.outputOverflows) parts.push(`${c.outputOverflows} out over`);
  return parts.join(', ');
}

export function StabilityTab() {
  const r = useStore((s) => s.results.stability);
  const o = useStore((s) => s.options.stability);
  const setOptions = useStore((s) => s.setOptions);
  const rate = useStore((s) => s.sampleRate);
  const detail = useStore((s) => s.outputDetail);
  const sizesText = o.bufferSizes.join(', ');

  const lowestClean = r?.rows.find((row) => row.verdict === 'clean');

  return (
    <div className="tab-body">
      <section className="card">
        <h3>Buffer stability</h3>
        <p className="muted">
          Opens the interface at each buffer size in turn, plays a chirp and then a steady tone, and checks three things: whether the host flagged an underrun or overrun, whether the recorded tone has a discontinuity or a dropout in it, and how long the loop actually is — the latency should grow with the buffer, and if it does not, the host did not honour the size. A run is <b>clean</b> when none of that happened.
        </p>
        <div className="options">
          <label className="field wide">
            <span className="field-label">Buffer sizes (frames)</span>
            <input
              type="text"
              value={sizesText}
              onChange={(e) => setOptions('stability', { bufferSizes: e.target.value.split(/[\s,]+/).map(Number).filter((v) => Number.isInteger(v) && v > 0) })}
            />
          </label>
          <NumberField label="Each for" unit="s" value={o.durationS} min={1} max={60} step={1} onChange={(v) => setOptions('stability', { durationS: v })} />
          <NumberField label="Tone" unit="Hz" value={o.frequency} min={20} max={20000} step={1} onChange={(v) => setOptions('stability', { frequency: v })} />
          <NumberField label="Level" unit="dBFS" value={o.levelDbfs} min={-60} max={0} step={1} onChange={(v) => setOptions('stability', { levelDbfs: v })} />
          <RunButton kind="stability" label="Run the sweep" />
        </div>
        {detail ? (
          <p className="muted small">
            {detail.bufferSizes.source}
            {detail.bufferSizes.min !== null ? ` — the driver allows ${detail.bufferSizes.min} to ${detail.bufferSizes.max}.` : ''}
          </p>
        ) : null}
      </section>
      {r ? (
        <>
          <div className="stats">
            <Stat label="Lowest clean buffer" value={lowestClean ? `${lowestClean.requestedFrames} frames` : 'none'} sub={lowestClean ? `${ms((lowestClean.requestedFrames / rate) * 1000)} — round trip ${ms(lowestClean.latencyMs)}` : 'every size tried had a problem'} tone={lowestClean ? 'good' : 'bad'} />
            <Stat label="Tried" value={`${r.rows.length} sizes`} sub={`${r.durationS} s each at ${fixed(r.frequency, 0)} Hz`} />
          </div>
          <table className="grid">
            <thead>
              <tr>
                <th>Requested</th>
                <th>Host used</th>
                <th>Verdict</th>
                <th>Round trip</th>
                <th>Driver claims</th>
                <th>Xruns</th>
                <th>Glitches</th>
                <th>Callbacks</th>
                <th>Worst gap</th>
                <th>Load</th>
              </tr>
            </thead>
            <tbody>
              {r.rows.map((row) => {
                const v = VERDICT[row.verdict];
                const g = row.glitches[0]?.analysis;
                return (
                  <tr key={row.requestedFrames} className={`verdict--${v.tone}`}>
                    <td>
                      {row.requestedFrames} <span className="muted">({ms((row.requestedFrames / rate) * 1000)})</span>
                    </td>
                    <td>{row.facts?.hostBufferFrames ?? (row.callbacks ? `${row.callbacks.minFrames}${row.callbacks.maxFrames !== row.callbacks.minFrames ? `–${row.callbacks.maxFrames}` : ''} per callback` : '—')}</td>
                    <td>
                      <span className={`badge badge--${v.tone}`}>{v.label}</span>
                      {row.error ? <div className="small muted">{row.error}</div> : null}
                    </td>
                    <td>{ms(row.latencyMs)}</td>
                    <td>{row.facts ? ms(row.facts.inputLatencyMs + row.facts.outputLatencyMs) : '—'}</td>
                    <td>{xruns(row)}</td>
                    <td>
                      {g ? (
                        g.discontinuities + g.dropouts === 0 ? (
                          <span>none {g.zeroRuns ? `(${g.zeroRuns} zero runs)` : ''}</span>
                        ) : (
                          <span>
                            {g.discontinuities ? `${g.discontinuities} discontinuit${g.discontinuities === 1 ? 'y' : 'ies'}` : ''}
                            {g.discontinuities && g.dropouts ? ', ' : ''}
                            {g.dropouts ? `${g.dropouts} dropout${g.dropouts === 1 ? '' : 's'}` : ''}
                            <div className="small muted">
                              {g.events
                                .slice(0, 4)
                                .map((e) => `${e.kind === 'dropout' ? 'gap' : 'jump'} at ${e.atS.toFixed(2)} s (${e.durationMs.toFixed(1)} ms, ${db(e.severityDb, 0)})`)
                                .join('; ')}
                              {g.events.length > 4 ? ` … ${g.events.length - 4} more` : ''}
                            </div>
                          </span>
                        )
                      ) : (
                        '—'
                      )}
                    </td>
                    <td>{row.callbacks ? `${row.callbacks.callbacks}${row.callbacks.lateCallbacks ? `, ${row.callbacks.lateCallbacks} late` : ''}` : '—'}</td>
                    <td>{row.callbacks ? `${ms(row.callbacks.maxIntervalMs)} / ${ms(row.callbacks.expectedIntervalMs)}` : '—'}</td>
                    <td>{row.callbacks ? `${(row.callbacks.cpuLoad * 100).toFixed(1)} %` : '—'}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {r.rows.find((row) => row.facts)?.facts ? <Facts facts={r.rows.find((row) => row.facts)!.facts!} /> : null}
          <p className="muted small">
            Residual is what is left after a sine is fitted to the recording: the interface's own noise and distortion. A glitch is a window where it jumps to many times the median, or where the fitted amplitude collapses. Worst gap is the longest time between two callbacks against the expected buffer period.
          </p>
        </>
      ) : null}
    </div>
  );
}
