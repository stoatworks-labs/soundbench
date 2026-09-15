import { useEffect } from 'react';

import { DeviceTab } from './components/DeviceTab';
import { LatencyTab } from './components/LatencyTab';
import { NoiseTab } from './components/NoiseTab';
import { ReportTab } from './components/ReportTab';
import { Setup } from './components/Setup';
import { StabilityTab } from './components/StabilityTab';
import { SweepTab } from './components/SweepTab';
import { ToneTab } from './components/ToneTab';
import { TransferTab } from './components/TransferTab';
import { api, inTauri } from './lib/ipc';
import { TEST_NAMES, TEST_ORDER, useStore, type Tab } from './store';

const TABS: { id: Tab; label: string }[] = [{ id: 'device', label: 'Device' }, ...TEST_ORDER.map((k) => ({ id: k as Tab, label: TEST_NAMES[k] })), { id: 'report', label: 'Report' }];

export function App() {
  const load = useStore((s) => s.load);
  const tab = useStore((s) => s.tab);
  const setTab = useStore((s) => s.setTab);
  const error = useStore((s) => s.error);
  const loading = useStore((s) => s.loading);
  const running = useStore((s) => s.running);
  const progress = useStore((s) => s.progress);
  const results = useStore((s) => s.results);
  const setProgress = useStore((s) => s.setProgress);
  const cancel = useStore((s) => s.cancel);

  useEffect(() => {
    void load();
    if (!inTauri) return;
    let un: (() => void) | undefined;
    void api.onProgress((p) => setProgress(p)).then((u) => {
      un = u;
    });
    return () => un?.();
  }, [load, setProgress]);

  return (
    <div className="app">
      <header className="top">
        <h1>
          Sound<span>Bench</span>
        </h1>
        <span className="tag">audio interface test bench</span>
        <span className="ver">{__APP_VERSION__}</span>
        <div className="spacer" />
        {!inTauri ? <span className="pill warn">not inside the desktop app — nothing here can reach an audio device</span> : null}
        <button type="button" className="btn small" data-stoatworks-about>
          About
        </button>
      </header>
      <div className="main">
        <Setup />
        <section className="content">
          <nav className="tabs">
            {TABS.map((t) => (
              <button key={t.id} type="button" className={`tab${tab === t.id ? ' tab--on' : ''}${t.id in results ? ' tab--done' : ''}${running === t.id ? ' tab--running' : ''}`} onClick={() => setTab(t.id)}>
                {t.label}
              </button>
            ))}
          </nav>
          {error ? (
            <div className="banner bad">
              {error}
              <button type="button" className="btn small" onClick={() => useStore.setState({ error: null })}>
                Dismiss
              </button>
            </div>
          ) : null}
          {loading ? <div className="banner">Scanning audio devices…</div> : null}
          {tab === 'device' ? <DeviceTab /> : null}
          {tab === 'latency' ? <LatencyTab /> : null}
          {tab === 'sweep' ? <SweepTab /> : null}
          {tab === 'tone' ? <ToneTab /> : null}
          {tab === 'noise' ? <NoiseTab /> : null}
          {tab === 'transfer' ? <TransferTab /> : null}
          {tab === 'stability' ? <StabilityTab /> : null}
          {tab === 'report' ? <ReportTab /> : null}
        </section>
      </div>
      <footer className="status">
        {running ? (
          <>
            <span className="running-dot" />
            <span>
              {TEST_NAMES[running]}
              {progress?.message ? ` — ${progress.message}` : ''}
            </span>
            <progress value={progress?.fraction ?? 0} max={1} />
            <button type="button" className="btn small danger" onClick={() => void cancel()}>
              Cancel
            </button>
          </>
        ) : (
          <span className="muted">Idle. One test at a time; each opens the interface, plays, records and closes it.</span>
        )}
      </footer>
    </div>
  );
}
