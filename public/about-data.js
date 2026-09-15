/*
 * Stoatworks Labs — About window data for SoundBench.
 *
 * Placeholder until stoatworks-backend/scripts/sync-about.py generates the
 * real one; the next sync overwrites it. The facts come from the website's
 * projects.json, which is the one place they are written down.
 *
 * `version` here is a fallback read from this repo's own manifest at sync
 * time. The build injects the real one and overrides this.
 */
window.STOATWORKS_ABOUT = Object.assign({
  "name": "SoundBench",
  "slug": "soundbench",
  "version": "v0.1.0",
  "hook": "Audio interface test bench",
  "licence": "MIT",
  "page": "https://stoatworks-labs.com/software/soundbench/",
  "repo": "https://github.com/stoatworks-labs/soundbench"
}, window.STOATWORKS_ABOUT || {});
