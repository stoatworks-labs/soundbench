import { describe, expect, it } from 'vitest';

import { db, fixed, hz, pct, samples } from './format';

describe('format', () => {
  it('uses a real minus sign', () => {
    expect(fixed(-3.14159, 2)).toBe('−3.14');
    expect(db(-6.02)).toBe('−6.0 dB');
  });
  it('floors the unmeasurable', () => {
    expect(db(-200)).toBe('< −150 dB');
  });
  it('scales hertz', () => {
    expect(hz(997)).toBe('997 Hz');
    expect(hz(20000)).toBe('20.0 kHz');
    expect(hz(1234)).toBe('1.23 kHz');
  });
  it('keeps small percentages readable', () => {
    expect(pct(0.00012)).toBe('0.00012 %');
    expect(pct(1.5)).toBe('1.500 %');
  });
  it('drops the fraction on whole sample counts', () => {
    expect(samples(768)).toBe('768 samples');
    expect(samples(768.4)).toBe('768.4 samples');
  });
});
