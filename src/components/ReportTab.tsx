/** Everything measured, as a document to keep. */
import { save } from '@tauri-apps/plugin-dialog';
import { useMemo, useState } from 'react';

import { inTauri, api } from '../lib/ipc';
import { reportJson, reportMarkdown, type ReportInput } from '../lib/report';
import { TEST_NAMES, TEST_ORDER, useStore } from '../store';

export function ReportTab() {
  const s = useStore();
  const [status, setStatus] = useState<string | null>(null);
  const input: ReportInput = useMemo(
    () => ({
      app: 'SoundBench',
      version: __APP_VERSION__,
      when: new Date(),
      snapshot: s.snapshot,
      target: s.target(),
      input: s.inputDetail,
      output: s.outputDetail,
      results: s.results,
    }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [s.snapshot, s.inputDetail, s.outputDetail, s.results, s.inputDevice, s.outputDevice, s.inputChannels, s.outputChannels, s.sampleRate, s.bufferFrames],
  );
  const md = useMemo(() => reportMarkdown(input), [input]);
  const done = TEST_ORDER.filter((k) => s.results[k]);
  const slug = (s.outputDetail?.info.name ?? 'interface').replace(/[^a-z0-9]+/gi, '-').replace(/^-|-$/g, '').toLowerCase();
  const stamp = new Date().toISOString().slice(0, 16).replace(/[:T]/g, '-');

  async function saveAs(ext: 'md' | 'json') {
    const text = ext === 'md' ? md : reportJson(input);
    const name = `soundbench-${slug}-${stamp}.${ext}`;
    if (!inTauri) {
      const blob = new Blob([text], { type: 'text/plain' });
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = name;
      a.click();
      return;
    }
    try {
      const path = await save({ defaultPath: name, filters: [{ name: ext === 'md' ? 'Markdown' : 'JSON', extensions: [ext] }] });
      if (!path) return;
      await api.saveText(path, text);
      setStatus(`Saved ${path}`);
    } catch (e) {
      setStatus(String(e));
    }
  }

  return (
    <div className="tab-body">
      <section className="card">
        <h3>Report</h3>
        <p className="muted">
          {done.length ? `Includes ${done.map((k) => TEST_NAMES[k]).join(', ')}.` : 'Nothing has been measured on this setup yet. Run a test, or the whole bench, and it appears here.'} The JSON carries every curve and every burst; the Markdown is the summary.
        </p>
        <div className="options">
          <button type="button" className="btn primary" onClick={() => void saveAs('md')}>
            Save Markdown…
          </button>
          <button type="button" className="btn" onClick={() => void saveAs('json')}>
            Save JSON…
          </button>
          <button
            type="button"
            className="btn"
            onClick={() => {
              void navigator.clipboard.writeText(md).then(() => setStatus('Copied the Markdown to the clipboard.'));
            }}
          >
            Copy Markdown
          </button>
          {status ? <span className="muted small">{status}</span> : null}
        </div>
      </section>
      <section className="card">
        <pre className="report">{md}</pre>
      </section>
    </div>
  );
}
