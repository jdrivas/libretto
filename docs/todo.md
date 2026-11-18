# What's next

The one list of what to do next, for this repo and for roon-rd. Each item points to where
its detail lives: `operas/<opera>/STATUS.md` (build history), `decisions.md` (why, and the
"To revisit" triggers), `repertoire-plan.md` (the order of operas).

Keep it current: when work finishes an item, tick it and move it to "Done" in the same
commit. When a session turns up something to do later, add it here. Don't let it wait for
the end of the session.

*Last updated 2026-11-17.*

## Now

- [ ] **D22: ensemble card.** Show a passage where two or more characters sing at once as one
  card, as the chorus is shown once (D10). Start with Solti's No. 21, disc 11 track 2,
  1:51-2:51. See decisions.md D22.
- [ ] **D17: full listening of Figaro**, after D22, on the touch display in libretto mode
  (roon-rd 1.5.0). Whatever it turns up becomes the fix list before Così.

## Figaro loose ends

- [ ] **app-no15** (No. 15 as recitative, appendix): a second recitative starts mid-system
  on p. 530; pieces are only split at system starts. STATUS.md, "P2 result".
- [ ] **app-no28a** (Rondo "Al desio", appendix): one bar short at p. 555 (printed 93,
  counted 92), probably the cadenza. STATUS.md, "P2 result".
- [ ] **Titles check:** the placement check that compares placed numbers with track titles
  should tell "the title doesn't mention this number" from "the title names a different
  number" (Solti's No. 9: Decca's title names Nos. 7 and 8). STATUS.md, "P4 / P5 result".
- [ ] **Solti's Sextet drifts** a little at times and recovers. Left as is; look again if the
  full listening shows other ensembles doing the same. STATUS.md, "P8 result".

## roon-rd

- [ ] **Next and mute do nothing on Bedroom dCS**, from the zone card and libretto mode
  alike (play works). The requests reach the server, but `control_zone` / `mute_output`
  (src/roon/mod.rs) ignore Roon's reply. Likely cause: Roon doesn't allow next at that
  moment (`is_next_allowed` false), and the output has no volume control. First log Roon's
  reply, `is_next_allowed` and the output's volume type, and have the user tap once. Then
  grey out the buttons Roon won't act on. Found 2026-11-17.

## Così checkpoint (before Così starts)

- [ ] **D7: gloss in the tool.** Discuss moving the verse list, phrase split and gloss from
  the session into the tool (Claude API).
- [ ] **D21 / D24: before any re-time of all pieces.** A full re-time would cut Solti's No. 9
  short (D21) and drop No. 24's kept end (D24).
- [ ] Then Così per repertoire-plan.md: "nothing new", so it measures what a second opera
  costs.

## Later

- [ ] **Where `libretto-work` lives**: it's Dropbox-ignored and local for now; revisit when
  the external disk arrives. Consider a separate data repo for the working files.

## Done

- 2026-11-17: roon-rd 1.5.0 released: the sung-text viewer, libretto track matching, and
  libretto mode on the main page.
- 2026-11-16: all of Figaro built, both recordings, 3,327 lines each; Act IV spot check
  passed.

## Dropped

These items from the February list were made obsolete by the pipeline (2026-11-17):
- automating the scaffold → timing overlay: placement now finds every number in the
  recording from the score;
- the Solti end-to-end test: done by P8 and the act loop;
- `group` tags in the base libretto and ensemble display: replaced by D22;
- recitative classification from track titles: the decode gives the pieces.
