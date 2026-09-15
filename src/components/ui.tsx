/** Small shared pieces: readout tiles, labelled fields, the run button, the facts strip. */
import type { ReactNode } from 'react';

import { framesMs, ms } from '../lib/format';
import { useStore } from '../store';
import type { StreamFacts, TestKind } from '../types';

export function Stat({ label, value, sub, tone }: { label: string; value: ReactNode; sub?: ReactNode; tone?: 'good' | 'warn' | 'bad' }) {
  return (
    <div className={`stat${tone ? ` stat--${tone}` : ''}`}>
      <div className="stat-label">{label}</div>
      <div className="stat-value">{value}</div>
      {sub ? <div className="stat-sub">{sub}</div> : null}
    </div>
  );
}

export function Field({ label, children, hint }: { label: string; children: ReactNode; hint?: string }) {
  return (
    <label className="field" title={hint}>
      <span className="field-label">{label}</span>
      {children}
    </label>
  );
}

export function NumberField({ label, value, onChange, step, min, max, unit, hint }: { label: string; value: number; onChange: (v: number) => void; step?: number; min?: number; max?: number; unit?: string; hint?: string }) {
  return (
    <Field label={unit ? `${label} (${unit})` : label} hint={hint}>
      <input
        type="number"
        value={value}
        step={step}
        min={min}
        max={max}
        onChange={(e) => {
          const v = Number(e.target.value);
          if (Number.isFinite(v)) onChange(v);
        }}
      />
    </Field>
  );
}

export function RunButton({ kind, label }: { kind: TestKind; label?: string }) {
  const running = useStore((s) => s.running);
  const run = useStore((s) => s.run);
  const cancel = useStore((s) => s.cancel);
  const ready = useStore((s) => s.inputDevice !== null && s.outputDevice !== null);
  if (running === kind) {
    return (
      <button type="button" className="btn danger" onClick={() => void cancel()}>
        Cancel
      </button>
    );
  }
  return (
    <button type="button" className="btn primary" disabled={!ready || running !== null} onClick={() => void run(kind)}>
      {label ?? 'Run'}
    </button>
  );
}

export function Facts({ facts }: { facts: StreamFacts }) {
  const rate = facts.sampleRate;
  return (
    <div className="facts">
      <span>
        Stream <b>{rate} Hz</b>, <b>{facts.bufferFrames || 'host-chosen'}</b> frames
        {facts.bufferFrames ? ` (${framesMs(facts.bufferFrames, rate)})` : ''}
      </span>
      {facts.hostBufferFrames !== null ? (
        <span title={facts.hostBufferSource ?? ''}>
          host buffer <b>{facts.hostBufferFrames}</b> frames
        </span>
      ) : null}
      <span>
        driver reports in <b>{ms(facts.inputLatencyMs)}</b> / out <b>{ms(facts.outputLatencyMs)}</b>
      </span>
      {facts.reportedLoopMs !== null ? (
        <span>
          ADC→DAC claim <b>{ms(facts.reportedLoopMs)}</b>
        </span>
      ) : null}
      <span>
        callback load <b>{(facts.cpuLoad * 100).toFixed(1)} %</b>
      </span>
    </div>
  );
}

export function Empty({ children }: { children: ReactNode }) {
  return <div className="empty">{children}</div>;
}
