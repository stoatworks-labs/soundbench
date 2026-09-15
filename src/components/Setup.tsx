/**
 * The left column: which host API, which devices, which channels, what
 * rate and buffer. Everything a test needs to know about *where* to run.
 */
import { devicesOfHost, TEST_NAMES, TEST_ORDER, useStore } from '../store';
import type { DeviceInfo } from '../types';
import { Field } from './ui';

function deviceLabel(d: DeviceInfo): string {
  const io = [d.maxInputChannels ? `${d.maxInputChannels} in` : '', d.maxOutputChannels ? `${d.maxOutputChannels} out` : ''].filter(Boolean).join(', ');
  return `${d.name} (${io})`;
}

function ChannelPicker({ count, selected, names, onToggle, label }: { count: number; selected: number[]; names: string[]; onToggle: (c: number) => void; label: string }) {
  if (count <= 0) return null;
  const many = count > 8;
  return (
    <div className="channels">
      <div className="field-label">{label}</div>
      <div className={`chips${many ? ' chips--dense' : ''}`}>
        {Array.from({ length: count }, (_, i) => (
          <button
            key={i}
            type="button"
            className={`chip${selected.includes(i) ? ' chip--on' : ''}`}
            title={names[i] ?? `Channel ${i + 1}`}
            onClick={() => onToggle(i)}
          >
            {many ? i + 1 : (names[i] ?? `Ch ${i + 1}`).replace(/^Channel /, 'Ch ')}
          </button>
        ))}
      </div>
    </div>
  );
}

export function Setup() {
  const s = useStore();
  const snap = s.snapshot;
  const hosts = snap?.hosts ?? [];
  const devices = devicesOfHost(snap, s.hostIndex);
  const inputs = devices.filter((d) => d.maxInputChannels > 0);
  const outputs = devices.filter((d) => d.maxOutputChannels > 0);
  const inDev = devices.find((d) => d.index === s.inputDevice) ?? null;
  const outDev = devices.find((d) => d.index === s.outputDevice) ?? null;
  const rates = (() => {
    const a = s.inputDetail?.sampleRates.filter((r) => r.input).map((r) => r.rate) ?? [];
    const b = s.outputDetail?.sampleRates.filter((r) => r.output).map((r) => r.rate) ?? [];
    const both = a.length && b.length ? a.filter((r) => b.includes(r)) : a.length ? a : b;
    const list = both.length ? both : [44100, 48000, 88200, 96000, 176400, 192000];
    return list.includes(s.sampleRate) ? list : [...list, s.sampleRate].sort((x, y) => x - y);
  })();
  const buffers = (() => {
    const c = s.outputDetail?.bufferSizes.candidates ?? s.inputDetail?.bufferSizes.candidates ?? [32, 64, 128, 256, 512, 1024, 2048];
    return c.includes(s.bufferFrames) ? c : [...c, s.bufferFrames].sort((x, y) => x - y);
  })();
  const busy = s.running !== null;
  const isMac = snap?.platform.startsWith('macOS') ?? false;
  const host = hosts.find((h) => h.index === s.hostIndex);
  const sameDevice = inDev && outDev && inDev.index === outDev.index;

  return (
    <aside className="setup">
      <Field label="Audio API" hint={host?.description}>
        <select value={s.hostIndex ?? ''} disabled={busy} onChange={(e) => s.setHost(e.target.value === '' ? null : Number(e.target.value))}>
          {hosts.map((h) => (
            <option key={h.kind + (h.index ?? 'x')} value={h.index ?? ''} disabled={!h.available}>
              {h.name}
              {h.available ? ` — ${h.deviceCount} device${h.deviceCount === 1 ? '' : 's'}` : ' — not in this build'}
            </option>
          ))}
        </select>
      </Field>
      {host?.note ? <p className="note">{host.note}</p> : null}
      {host?.description ? <p className="note muted">{host.description}</p> : null}

      <Field label="Output (plays the test signal)">
        <select value={s.outputDevice ?? ''} disabled={busy} onChange={(e) => void s.setOutputDevice(e.target.value === '' ? null : Number(e.target.value))}>
          <option value="">— choose —</option>
          {outputs.map((d) => (
            <option key={d.index} value={d.index}>
              {deviceLabel(d)}
              {d.isDefaultOutput ? ' • default' : ''}
            </option>
          ))}
        </select>
      </Field>
      <ChannelPicker
        count={outDev?.maxOutputChannels ?? 0}
        selected={s.outputChannels}
        names={s.outputDetail?.outputChannelNames ?? []}
        onToggle={s.toggleOutputChannel}
        label="Send on"
      />

      <Field label="Input (records what comes back)">
        <select value={s.inputDevice ?? ''} disabled={busy} onChange={(e) => void s.setInputDevice(e.target.value === '' ? null : Number(e.target.value))}>
          <option value="">— choose —</option>
          {inputs.map((d) => (
            <option key={d.index} value={d.index}>
              {deviceLabel(d)}
              {d.isDefaultInput ? ' • default' : ''}
            </option>
          ))}
        </select>
      </Field>
      <ChannelPicker
        count={inDev?.maxInputChannels ?? 0}
        selected={s.inputChannels}
        names={s.inputDetail?.inputChannelNames ?? []}
        onToggle={s.toggleInputChannel}
        label="Listen on"
      />
      {inDev && outDev && !sameDevice ? (
        <p className="note warn">Input and output are different devices. They run on separate clocks; expect the latency to drift and the tone's frequency to read off by their difference in ppm.</p>
      ) : null}

      <div className="row2">
        <Field label="Sample rate">
          <select value={s.sampleRate} disabled={busy} onChange={(e) => s.setSampleRate(Number(e.target.value))}>
            {rates.map((r) => (
              <option key={r} value={r}>
                {r} Hz
              </option>
            ))}
          </select>
        </Field>
        <Field label="Buffer" hint={s.outputDetail?.bufferSizes.source}>
          <select value={s.bufferFrames} disabled={busy} onChange={(e) => s.setBufferFrames(Number(e.target.value))}>
            {buffers.map((b) => (
              <option key={b} value={b}>
                {b} frames ({((b / s.sampleRate) * 1000).toFixed(2)} ms)
              </option>
            ))}
          </select>
        </Field>
      </div>

      {host?.kind === 'wasapi' ? (
        <label className="check">
          <input type="checkbox" checked={s.wasapiExclusive} disabled={busy} onChange={(e) => s.setWasapiExclusive(e.target.checked)} />
          Exclusive mode (bypass the Windows mixer)
        </label>
      ) : null}
      {isMac && host?.kind === 'core-audio' ? (
        <label className="check" title="Sets the device's own sample rate and buffer size for the test, and fails rather than resamples if it cannot. Off, PortAudio converts and the buffer size only applies to its own buffering.">
          <input type="checkbox" checked={s.macChangeDevice} disabled={busy} onChange={(e) => s.setMacChangeDevice(e.target.checked)} />
          Set the device's rate and buffer size
        </label>
      ) : null}

      <div className="setup-actions">
        <button type="button" className="btn primary wide" disabled={busy || s.inputDevice === null || s.outputDevice === null} onClick={() => void s.runAll()}>
          Run the whole bench
        </button>
        <div className="queue">
          {TEST_ORDER.map((k) => (
            <span key={k} className={`queue-item${s.results[k] ? ' done' : ''}${s.running === k ? ' active' : ''}${s.queue.includes(k) ? ' queued' : ''}`}>
              {TEST_NAMES[k]}
            </span>
          ))}
        </div>
        <button type="button" className="btn wide" disabled={busy} onClick={() => void s.rescan()}>
          Rescan devices
        </button>
      </div>

      <p className="note muted">
        Connect an output of the interface to an input with a cable. Every test except the noise floor plays through that loop. Keep the interface's own gain moderate: the test signals sit at −12 dBFS unless you change them.
      </p>
    </aside>
  );
}
