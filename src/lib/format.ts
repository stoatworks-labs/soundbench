/** Number formatting for readouts. Every readout goes through one of these. */

export function fixed(v: number | null | undefined, digits = 2, unit = ''): string {
  if (v === null || v === undefined || Number.isNaN(v)) return '—';
  if (!Number.isFinite(v)) return v > 0 ? '∞' : '−∞';
  const s = v.toFixed(digits).replace('-', '−');
  return unit ? `${s} ${unit}` : s;
}

export function db(v: number | null | undefined, digits = 1): string {
  if (v === null || v === undefined || Number.isNaN(v)) return '—';
  if (v <= -150) return `< −150 dB`;
  return fixed(v, digits, 'dB');
}

export function dbfs(v: number | null | undefined, digits = 1): string {
  if (v === null || v === undefined || Number.isNaN(v)) return '—';
  if (v <= -150) return `< −150 dBFS`;
  return fixed(v, digits, 'dBFS');
}

export function ms(v: number | null | undefined, digits = 2): string {
  return fixed(v, digits, 'ms');
}

export function pct(v: number | null | undefined): string {
  if (v === null || v === undefined || Number.isNaN(v)) return '—';
  if (v < 0.001) return `${fixed(v, 5)} %`;
  if (v < 0.1) return `${fixed(v, 4)} %`;
  return `${fixed(v, 3)} %`;
}

export function hz(v: number | null | undefined): string {
  if (v === null || v === undefined || Number.isNaN(v)) return '—';
  if (v >= 1000) return `${fixed(v / 1000, v >= 10000 ? 1 : 2)} kHz`;
  return `${fixed(v, v < 100 ? 1 : 0)} Hz`;
}

export function samples(v: number | null | undefined): string {
  if (v === null || v === undefined || Number.isNaN(v)) return '—';
  return `${fixed(v, Math.abs(v - Math.round(v)) < 0.005 ? 0 : 1)} samples`;
}

/** Frames at a rate, as ms. */
export function framesMs(frames: number, rate: number): string {
  return rate > 0 ? ms((frames / rate) * 1000) : '—';
}
