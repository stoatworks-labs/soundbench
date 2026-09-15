//! Window functions.
//!
//! Two are enough for this bench. The four-term Blackman-Harris has −92 dB
//! sidelobes, which is what lets a −100 dB harmonic be read next to a 0 dB
//! fundamental; its main lobe is ±4 bins wide, which is why [`tone`](crate::tone)
//! sums that many bins around each peak. The Hann half-windows taper the edges
//! of impulse-response segments before they are transformed.

use std::f64::consts::PI;

/// Four-term Blackman-Harris, periodic form.
pub fn blackman_harris(n: usize) -> Vec<f64> {
    let a = [0.35875, 0.48829, 0.14128, 0.01168];
    (0..n)
        .map(|i| {
            let x = 2.0 * PI * i as f64 / n as f64;
            a[0] - a[1] * x.cos() + a[2] * (2.0 * x).cos() - a[3] * (3.0 * x).cos()
        })
        .collect()
}

/// Albrecht's seven-term cosine-sum window (a "7-term Blackman-Harris"):
/// sidelobes below −180 dB, main lobe ±7 bins. The window for reading a
/// −120 dB harmonic beside a 0 dB fundamental; the four-term window's −92 dB
/// sidelobes sum to a THD+N floor near −88 dB, which is worse than a decent
/// interface.
pub fn blackman_harris_7(n: usize) -> Vec<f64> {
    let a = [
        0.27105140069342,
        0.43329793923448,
        0.21812299954311,
        0.06592544638803,
        0.01081174209837,
        0.00077658482522,
        0.00001388721735,
    ];
    (0..n)
        .map(|i| {
            let x = 2.0 * PI * i as f64 / n as f64;
            a.iter()
                .enumerate()
                .map(|(k, &ak)| if k % 2 == 0 { ak * (k as f64 * x).cos() } else { -ak * (k as f64 * x).cos() })
                .sum()
        })
        .collect()
}

/// Hann, periodic form.
pub fn hann(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos())
        .collect()
}

/// Sum of the window samples — the coherent gain, for scaling a spectrum so a
/// full-scale sine reads its true amplitude.
pub fn coherent_gain(w: &[f64]) -> f64 {
    w.iter().sum()
}

/// Sum of squares — the incoherent (power) gain, for scaling noise power.
pub fn power_gain(w: &[f64]) -> f64 {
    w.iter().map(|v| v * v).sum()
}

/// A flat window with raised-cosine tapers of `pre` samples at the start and
/// `post` samples at the end. Used to cut a segment of an impulse response
/// without the cut itself becoming a broadband click.
pub fn tukey_asym(n: usize, pre: usize, post: usize) -> Vec<f64> {
    let pre = pre.min(n / 2);
    let post = post.min(n - pre);
    (0..n)
        .map(|i| {
            if i < pre {
                0.5 - 0.5 * (PI * i as f64 / pre as f64).cos()
            } else if i >= n - post {
                let j = n - 1 - i;
                0.5 - 0.5 * (PI * j as f64 / post as f64).cos()
            } else {
                1.0
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_peak_at_one_and_taper_to_zero() {
        let w = blackman_harris(1024);
        assert!(w[0].abs() < 1e-4);
        let max = w.iter().cloned().fold(0.0, f64::max);
        assert!((max - 1.0).abs() < 1e-3);
        let w7 = blackman_harris_7(4096);
        assert!(w7[0].abs() < 1e-6);
        let max7 = w7.iter().cloned().fold(0.0, f64::max);
        assert!((max7 - 1.0).abs() < 1e-3, "{max7}");
        let h = hann(64);
        assert!(h[0].abs() < 1e-12);
        assert!((h[32] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn tukey_is_flat_in_the_middle() {
        let w = tukey_asym(100, 10, 20);
        assert_eq!(w[50], 1.0);
        assert!(w[0] < 1e-12);
        assert!(w[99] < 1e-12);
        assert!(w[9] > 0.9);
        assert!(w[80] > 0.9);
    }
}
