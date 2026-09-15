//! Real FFT helpers over `realfft`.
//!
//! Every transform in the crate goes through these three functions so the
//! scaling convention is in one place: `forward` is unscaled (bin k holds
//! Σ x[n]·e^(−2πikn/N)), `inverse` divides by N, so `inverse(forward(x)) == x`.

use realfft::num_complex::Complex;
use realfft::RealFftPlanner;

pub type C64 = Complex<f64>;

/// Smallest power of two that is at least `n`.
pub fn next_pow2(n: usize) -> usize {
    let mut p = 1;
    while p < n {
        p <<= 1;
    }
    p
}

/// Forward real FFT of `x`, zero-padded to `len` (which must be even and at
/// least `x.len()`). Returns `len/2 + 1` bins.
pub fn forward(x: &[f64], len: usize) -> Vec<C64> {
    assert!(len >= x.len() && len % 2 == 0, "fft length {len} for {} samples", x.len());
    let mut planner = RealFftPlanner::<f64>::new();
    let r2c = planner.plan_fft_forward(len);
    let mut input = vec![0.0f64; len];
    input[..x.len()].copy_from_slice(x);
    let mut output = r2c.make_output_vec();
    r2c.process(&mut input, &mut output).expect("fft sizes match");
    output
}

/// Inverse real FFT: `len/2 + 1` bins back to `len` samples, scaled by 1/N.
pub fn inverse(bins: &[C64], len: usize) -> Vec<f64> {
    assert_eq!(bins.len(), len / 2 + 1);
    let mut planner = RealFftPlanner::<f64>::new();
    let c2r = planner.plan_fft_inverse(len);
    let mut input = bins.to_vec();
    // realfft insists the DC and Nyquist bins are real; they are, up to
    // rounding, for anything that came out of `forward`.
    input[0].im = 0.0;
    input[len / 2].im = 0.0;
    let mut output = c2r.make_output_vec();
    c2r.process(&mut input, &mut output).expect("fft sizes match");
    let scale = 1.0 / len as f64;
    output.iter_mut().for_each(|v| *v *= scale);
    output
}

/// Linear convolution of `a` and `b` by FFT; result length `a.len() + b.len() − 1`.
pub fn convolve(a: &[f64], b: &[f64]) -> Vec<f64> {
    let n = a.len() + b.len() - 1;
    let len = next_pow2(n);
    let fa = forward(a, len);
    let fb = forward(b, len);
    let prod: Vec<C64> = fa.iter().zip(&fb).map(|(x, y)| x * y).collect();
    let mut out = inverse(&prod, len);
    out.truncate(n);
    out
}

/// Cross-correlation of `x` with `template`: `out[k] = Σ x[n+k]·t[n]`, for
/// `k` in `0..x.len()`. A peak at `k` means the template starts `k` samples
/// into `x`. Computed by FFT, so a 30-second recording against a 50 ms
/// template costs one transform rather than a billion multiplies.
pub fn xcorr(x: &[f64], template: &[f64]) -> Vec<f64> {
    let n = x.len() + template.len();
    let len = next_pow2(n);
    let fx = forward(x, len);
    let ft = forward(template, len);
    let prod: Vec<C64> = fx.iter().zip(&ft).map(|(a, b)| a * b.conj()).collect();
    let mut out = inverse(&prod, len);
    out.truncate(x.len());
    out
}

/// Cross-correlation with phase-transform (GCC-PHAT) weighting: the cross
/// spectrum is normalised to unit magnitude before the inverse transform, so
/// the result is a sharp peak at the lag regardless of how the signals are
/// coloured or band-limited. The plain [`xcorr`] is the better estimator
/// for a designed pulse; this is the one for aligning noise through an
/// unknown filter.
pub fn xcorr_phat(x: &[f64], template: &[f64]) -> Vec<f64> {
    let n = x.len() + template.len();
    let len = next_pow2(n);
    let fx = forward(x, len);
    let ft = forward(template, len);
    let prod: Vec<C64> = fx
        .iter()
        .zip(&ft)
        .map(|(a, b)| {
            let p = a * b.conj();
            let m = p.norm();
            if m > 1e-12 {
                p / m
            } else {
                C64::new(0.0, 0.0)
            }
        })
        .collect();
    let mut out = inverse(&prod, len);
    out.truncate(x.len());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_is_identity() {
        let x: Vec<f64> = (0..64).map(|i| ((i * 7) % 11) as f64 - 5.0).collect();
        let bins = forward(&x, 64);
        let back = inverse(&bins, 64);
        for (a, b) in x.iter().zip(&back) {
            assert!((a - b).abs() < 1e-9);
        }
    }

    #[test]
    fn convolution_matches_direct() {
        let a = [1.0, 2.0, 3.0];
        let b = [0.5, -1.0];
        let c = convolve(&a, &b);
        let expect = [0.5, 0.0, -0.5, -3.0];
        for (x, y) in c.iter().zip(&expect) {
            assert!((x - y).abs() < 1e-9, "{c:?}");
        }
    }

    #[test]
    fn phat_finds_the_offset_through_a_filter() {
        // Pink-ish noise through a heavy low-pass: the plain correlation peak
        // is broad, the PHAT peak is not.
        let x = crate::signal::pink_noise(4000, 0.1, 2);
        let xd: Vec<f64> = x.iter().map(|&v| v as f64).collect();
        let mut y = vec![0.0f64; 6000];
        let mut state = 0.0;
        for i in 0..4000 {
            state += 0.05 * (xd[i] - state);
            y[i + 300] = state;
        }
        let c = xcorr_phat(&y, &xd);
        let (k, _) = c[..2000].iter().enumerate().fold((0, f64::MIN), |m, (i, &v)| if v > m.1 { (i, v) } else { m });
        assert!((k as i64 - 300).abs() <= 3, "peak at {k}");
    }

    #[test]
    fn xcorr_finds_the_offset() {
        let t = [1.0, -2.0, 3.0, -1.0];
        let mut x = vec![0.0; 100];
        x[37..41].copy_from_slice(&t);
        let c = xcorr(&x, &t);
        let (k, _) = c.iter().enumerate().fold((0, f64::MIN), |m, (i, &v)| if v > m.1 { (i, v) } else { m });
        assert_eq!(k, 37);
    }
}
