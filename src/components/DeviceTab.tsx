/** Everything the driver says about the selected devices. */
import { ms } from '../lib/format';
import { api, inTauri } from '../lib/ipc';
import { useStore } from '../store';
import type { DeviceDetail } from '../types';
import { Empty } from './ui';

function DetailCard({ title, detail }: { title: string; detail: DeviceDetail | null }) {
  if (!detail) return <Empty>{title}: no device selected.</Empty>;
  const d = detail.info;
  const rates = detail.sampleRates.filter((r) => r.input || r.output);
  const bs = detail.bufferSizes;
  return (
    <section className="card">
      <h3>
        {title}: {d.name}
        <span className="pill">{d.hostName}</span>
      </h3>
      <table className="kv">
        <tbody>
          <tr>
            <th>Channels</th>
            <td>
              {d.maxInputChannels} in, {d.maxOutputChannels} out
            </td>
          </tr>
          <tr>
            <th>Default sample rate</th>
            <td>{d.defaultSampleRate} Hz</td>
          </tr>
          <tr>
            <th>Sample rates</th>
            <td>
              {rates.length ? (
                <span className="chips">
                  {rates.map((r) => (
                    <span key={r.rate} className={`chip chip--static${r.duplex || (r.input && !d.maxOutputChannels) || (r.output && !d.maxInputChannels) ? ' chip--on' : ' chip--half'}`} title={`in ${r.input ? 'yes' : 'no'}, out ${r.output ? 'yes' : 'no'}, duplex ${r.duplex ? 'yes' : 'no'}`}>
                      {r.rate}
                    </span>
                  ))}
                </span>
              ) : (
                'none of the probed rates'
              )}
            </td>
          </tr>
          <tr>
            <th>Buffer sizes</th>
            <td>
              {bs.min !== null ? `${bs.min}–${bs.max} frames` : 'range not published'}
              {bs.preferred ? `, preferred ${bs.preferred}` : ''}
              {bs.granularity !== null ? `, granularity ${bs.granularity === -1 ? 'powers of two' : bs.granularity}` : ''}
              <div className="muted small">{bs.source}</div>
            </td>
          </tr>
          <tr>
            <th>Driver's latency claim</th>
            <td>
              in {ms(d.defaultLowInputLatencyMs)} – {ms(d.defaultHighInputLatencyMs)}, out {ms(d.defaultLowOutputLatencyMs)} – {ms(d.defaultHighOutputLatencyMs)}
              <div className="muted small">PortAudio's default low/high latency for this device, as the host API reports it.</div>
            </td>
          </tr>
          {d.maxInputChannels > 0 ? (
            <tr>
              <th>Input channels</th>
              <td className="names">{detail.inputChannelNames.join(' · ')}</td>
            </tr>
          ) : null}
          {d.maxOutputChannels > 0 ? (
            <tr>
              <th>Output channels</th>
              <td className="names">{detail.outputChannelNames.join(' · ')}</td>
            </tr>
          ) : null}
          {detail.platform.map((kv) => (
            <tr key={kv.key}>
              <th>{kv.key}</th>
              <td>{kv.value}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {detail.notes.map((n) => (
        <p key={n} className="note warn">
          {n}
        </p>
      ))}
      {d.hostKind === 'asio' && inTauri ? (
        <button type="button" className="btn" onClick={() => api.showAsioPanel(d.index).catch((e) => alert(String(e)))}>
          Open the ASIO control panel
        </button>
      ) : null}
    </section>
  );
}

export function DeviceTab() {
  const snap = useStore((s) => s.snapshot);
  const inputDetail = useStore((s) => s.inputDetail);
  const outputDetail = useStore((s) => s.outputDetail);
  const sameDevice = inputDetail && outputDetail && inputDetail.info.index === outputDetail.info.index;
  return (
    <div className="tab-body">
      <div className="facts">
        <span>{snap?.portaudio}</span>
        <span>{snap?.platform}</span>
        <span>
          {snap?.hosts.filter((h) => h.available).length ?? 0} audio APIs, {snap?.devices.length ?? 0} devices
        </span>
      </div>
      {sameDevice ? <DetailCard title="Interface" detail={outputDetail} /> : (
        <>
          <DetailCard title="Output" detail={outputDetail} />
          <DetailCard title="Input" detail={inputDetail} />
        </>
      )}
      <section className="card">
        <h3>Audio APIs on this machine</h3>
        <table className="kv">
          <tbody>
            {snap?.hosts.map((h) => (
              <tr key={h.kind + (h.index ?? '')}>
                <th>
                  {h.name}
                  {h.isDefault ? ' (default)' : ''}
                </th>
                <td>
                  {h.available ? `${h.deviceCount} devices. ` : `${h.note ?? 'not available'} `}
                  <span className="muted">{h.description}</span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </div>
  );
}
