/**
 * A canvas line plot: log or linear x, linear y, several series, a hover
 * readout. Deliberately small — the app has six kinds of plot and they all
 * want the same axes, the same grid and the same cursor, so there is one.
 */
import { useEffect, useRef, useState } from 'react';

export interface Series {
  name: string;
  x: number[];
  y: number[];
  color: string;
  /** Draw as a dashed line. */
  dashed?: boolean;
  width?: number;
}

export interface PlotProps {
  series: Series[];
  xLog?: boolean;
  xLabel?: string;
  yLabel?: string;
  xRange?: [number, number];
  yRange?: [number, number];
  height?: number;
  yFormat?: (v: number) => string;
  xFormat?: (v: number) => string;
  /** Horizontal reference lines, e.g. 0 dB. */
  yMarks?: number[];
}

const PAD = { l: 54, r: 14, t: 12, b: 30 };

function niceStep(range: number, target: number): number {
  const raw = range / target;
  const mag = Math.pow(10, Math.floor(Math.log10(raw)));
  for (const m of [1, 2, 2.5, 5, 10]) {
    if (raw <= m * mag) return m * mag;
  }
  return 10 * mag;
}

function fmtHz(v: number): string {
  if (v >= 1000) return `${v / 1000}k`;
  return `${v}`;
}

export function Plot({ series, xLog = false, xLabel, yLabel, xRange, yRange, height = 260, yFormat, xFormat, yMarks }: PlotProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<number | null>(null);
  const [size, setSize] = useState({ w: 600, h: height });

  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      const r = entries[0].contentRect;
      setSize({ w: Math.max(200, r.width), h: height });
    });
    ro.observe(el);
    setSize({ w: Math.max(200, el.clientWidth), h: height });
    return () => ro.disconnect();
  }, [height]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(size.w * dpr);
    canvas.height = Math.round(size.h * dpr);
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const W = size.w;
    const H = size.h;
    ctx.clearRect(0, 0, W, H);

    const all = series.filter((s) => s.x.length > 0);
    let x0 = xRange?.[0] ?? Math.min(...all.map((s) => Math.min(...s.x.filter(Number.isFinite))));
    let x1 = xRange?.[1] ?? Math.max(...all.map((s) => Math.max(...s.x.filter(Number.isFinite))));
    if (!Number.isFinite(x0) || !Number.isFinite(x1) || x0 === x1) {
      x0 = xLog ? 20 : 0;
      x1 = xLog ? 20000 : 1;
    }
    let y0: number;
    let y1: number;
    if (yRange) {
      [y0, y1] = yRange;
    } else {
      const ys = all.flatMap((s) => s.y.filter(Number.isFinite));
      y0 = ys.length ? Math.min(...ys) : 0;
      y1 = ys.length ? Math.max(...ys) : 1;
      if (y0 === y1) {
        y0 -= 1;
        y1 += 1;
      }
      const m = (y1 - y0) * 0.08;
      y0 -= m;
      y1 += m;
    }
    const pw = W - PAD.l - PAD.r;
    const ph = H - PAD.t - PAD.b;
    const lx0 = xLog ? Math.log10(x0) : x0;
    const lx1 = xLog ? Math.log10(x1) : x1;
    const px = (x: number) => PAD.l + (((xLog ? Math.log10(x) : x) - lx0) / (lx1 - lx0)) * pw;
    const py = (y: number) => PAD.t + ((y1 - y) / (y1 - y0)) * ph;

    const css = getComputedStyle(canvas);
    const line = css.getPropertyValue('--line').trim() || '#262c38';
    const muted = css.getPropertyValue('--muted').trim() || '#8e99ab';
    const text = css.getPropertyValue('--text').trim() || '#e6ebf2';
    ctx.font = '11px system-ui, sans-serif';

    // Grid: x
    ctx.strokeStyle = line;
    ctx.fillStyle = muted;
    ctx.lineWidth = 1;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'top';
    if (xLog) {
      const d0 = Math.floor(Math.log10(x0));
      const d1 = Math.ceil(Math.log10(x1));
      for (let d = d0; d <= d1; d++) {
        for (const m of [1, 2, 3, 4, 5, 6, 7, 8, 9]) {
          const v = m * Math.pow(10, d);
          if (v < x0 || v > x1) continue;
          const x = px(v);
          ctx.globalAlpha = m === 1 ? 1 : 0.45;
          ctx.beginPath();
          ctx.moveTo(x, PAD.t);
          ctx.lineTo(x, PAD.t + ph);
          ctx.stroke();
          ctx.globalAlpha = 1;
          if (m === 1 || m === 2 || m === 5) ctx.fillText(xFormat ? xFormat(v) : fmtHz(v), x, PAD.t + ph + 4);
        }
      }
    } else {
      const step = niceStep(x1 - x0, 8);
      for (let v = Math.ceil(x0 / step) * step; v <= x1 + 1e-9; v += step) {
        const x = px(v);
        ctx.beginPath();
        ctx.moveTo(x, PAD.t);
        ctx.lineTo(x, PAD.t + ph);
        ctx.stroke();
        ctx.fillText(xFormat ? xFormat(v) : `${+v.toFixed(6)}`, x, PAD.t + ph + 4);
      }
    }
    // Grid: y
    ctx.textAlign = 'right';
    ctx.textBaseline = 'middle';
    const ystep = niceStep(y1 - y0, 6);
    for (let v = Math.ceil(y0 / ystep) * ystep; v <= y1 + 1e-9; v += ystep) {
      const y = py(v);
      ctx.beginPath();
      ctx.moveTo(PAD.l, y);
      ctx.lineTo(PAD.l + pw, y);
      ctx.stroke();
      ctx.fillText(yFormat ? yFormat(v) : `${+v.toFixed(6)}`, PAD.l - 6, y);
    }
    for (const m of yMarks ?? []) {
      if (m < y0 || m > y1) continue;
      ctx.strokeStyle = muted;
      ctx.setLineDash([4, 4]);
      ctx.beginPath();
      ctx.moveTo(PAD.l, py(m));
      ctx.lineTo(PAD.l + pw, py(m));
      ctx.stroke();
      ctx.setLineDash([]);
    }
    // Axis labels
    ctx.fillStyle = muted;
    if (xLabel) {
      ctx.textAlign = 'right';
      ctx.textBaseline = 'bottom';
      ctx.fillText(xLabel, PAD.l + pw, H - 2);
    }
    if (yLabel) {
      ctx.save();
      ctx.translate(12, PAD.t);
      ctx.rotate(-Math.PI / 2);
      ctx.textAlign = 'right';
      ctx.textBaseline = 'middle';
      ctx.fillText(yLabel, 0, 0);
      ctx.restore();
    }

    // Series
    ctx.save();
    ctx.beginPath();
    ctx.rect(PAD.l, PAD.t, pw, ph);
    ctx.clip();
    for (const s of series) {
      ctx.strokeStyle = s.color;
      ctx.lineWidth = s.width ?? 1.6;
      ctx.setLineDash(s.dashed ? [5, 4] : []);
      ctx.beginPath();
      let pen = false;
      for (let i = 0; i < s.x.length; i++) {
        const xv = s.x[i];
        const yv = s.y[i];
        if (!Number.isFinite(xv) || !Number.isFinite(yv) || (xLog && xv <= 0)) {
          pen = false;
          continue;
        }
        const x = px(xv);
        const y = py(yv);
        if (!pen) {
          ctx.moveTo(x, y);
          pen = true;
        } else {
          ctx.lineTo(x, y);
        }
      }
      ctx.stroke();
    }
    ctx.setLineDash([]);
    ctx.restore();

    // Legend
    let lx = PAD.l + 8;
    ctx.textAlign = 'left';
    ctx.textBaseline = 'top';
    for (const s of series) {
      ctx.fillStyle = s.color;
      ctx.fillRect(lx, PAD.t + 6, 10, 3);
      ctx.fillStyle = text;
      ctx.fillText(s.name, lx + 14, PAD.t + 2);
      lx += 14 + ctx.measureText(s.name).width + 14;
    }

    // Hover
    if (hover !== null && hover >= PAD.l && hover <= PAD.l + pw) {
      const xv = xLog ? Math.pow(10, lx0 + ((hover - PAD.l) / pw) * (lx1 - lx0)) : lx0 + ((hover - PAD.l) / pw) * (lx1 - lx0);
      ctx.strokeStyle = muted;
      ctx.beginPath();
      ctx.moveTo(hover, PAD.t);
      ctx.lineTo(hover, PAD.t + ph);
      ctx.stroke();
      const lines: string[] = [xFormat ? xFormat(xv) : xLog ? `${xv >= 1000 ? (xv / 1000).toFixed(2) + ' kHz' : xv.toFixed(1) + ' Hz'}` : `${+xv.toFixed(4)}`];
      for (const s of series) {
        // nearest index
        let best = -1;
        let bd = Infinity;
        for (let i = 0; i < s.x.length; i++) {
          const d = Math.abs((xLog ? Math.log10(s.x[i]) : s.x[i]) - (xLog ? Math.log10(xv) : xv));
          if (d < bd) {
            bd = d;
            best = i;
          }
        }
        if (best >= 0 && Number.isFinite(s.y[best])) lines.push(`${s.name}: ${yFormat ? yFormat(s.y[best]) : (+s.y[best].toFixed(3)).toString()}`);
      }
      const bw = Math.max(...lines.map((l) => ctx.measureText(l).width)) + 12;
      const bh = lines.length * 14 + 8;
      const bx = hover + 10 + bw > W ? hover - 10 - bw : hover + 10;
      const by = PAD.t + 20;
      ctx.fillStyle = 'rgba(11, 13, 18, 0.92)';
      ctx.fillRect(bx, by, bw, bh);
      ctx.strokeStyle = line;
      ctx.strokeRect(bx, by, bw, bh);
      ctx.fillStyle = text;
      ctx.textAlign = 'left';
      ctx.textBaseline = 'top';
      lines.forEach((l, i) => ctx.fillText(l, bx + 6, by + 4 + i * 14));
    }
  }, [series, xLog, xLabel, yLabel, xRange, yRange, size, hover, yFormat, xFormat, yMarks]);

  return (
    <div ref={wrapRef} className="plot" style={{ height }}>
      <canvas
        ref={canvasRef}
        style={{ width: size.w, height: size.h }}
        onMouseMove={(e) => setHover(e.nativeEvent.offsetX)}
        onMouseLeave={() => setHover(null)}
      />
    </div>
  );
}
