# Libretto TODO

## Pipeline Automation

- [ ] **Automate scaffold → timing overlay**: Fetch track listings from external sources (Wikipedia, MusicBrainz, Discogs) to populate track titles, disc/track numbers, durations, and number_ids — replacing the current manual edit of the `timing init` scaffold output.

## Viewer / roon-rd Integration

- [ ] **Ensemble group display**: Update the libretto viewer to render grouped segments (same `group` tag) together — needed for duets, trios, ensembles where multiple characters sing simultaneously.
- [ ] **Test full pipeline with Solti data**: Run resolve → estimate → merge → roon-rd viewer end-to-end and verify timing accuracy during playback.

## Data Quality

- [ ] **Populate ensemble groups in base libretto**: Add `group` tags to the Figaro base libretto for duets/trios/ensembles so simultaneous lines display together.
- [ ] **Improve recitative classification**: Currently inferred from track title keywords; consider enriching the base libretto with more accurate section-level type annotations.
