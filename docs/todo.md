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
- [ ] **Point roon-rd's `config.toml` at the moved `libretto-work`** (D27). Line 18 has
  pointed at a February file since then; set `libretto_dir` to
  `/Volumes/JDR Desktop Attached/Google Drive/Projects/roon-rd/Libretto/libretto-work/figaro/interchange`.
  It's a roon-rd commit, which needs the user's approval. Until then, launch with
  `--libretto-dir "<that path>"` (quoted: the path has spaces). roon-rd's `.claude`
  settings were switched to the new paths on 2026-11-18.

## Così checkpoint (before Così starts)

- [ ] **New-opera model: discuss and decide.** Proposal in `docs/new-opera-model.md`:
  checkpoints G0-G7, a definition of "done", and the code changes P1-P8. Its section 6
  lists the decisions. Così fails at `add-recording` today: the file names' Unicode form
  and "Act 1" titles (section 4.1).
- [ ] **Decide: are `score/<piece>.json` files licensed?** They hold every pitch and sung
  syllable, as much of the score as `decode/pieces/`, but the licence rule names only the
  latter. Once decided: a D-entry, and the rule in `CLAUDE.md`. new-opera-model.md A.7.
- [ ] **Interchange format 1.2, with P7** (new-opera-model.md A.11, "Quirks"):
  - `act` as an integer, as in every other file;
  - one track id form (the pipeline's `d09t01` against the interchange's `d9t1`);
  - `translation_language` from opera.toml, not hard-coded "en";
  - `opera.title` without the composer;
  - add `opera.id` and `recording.id`;
  - stop repeating `verses` in every segment (most of the 3.7 MB);
  - a recording with no disc tags shouldn't be published as disc 1.
- [ ] **With P4: name each file's clock.** `start`/`end` mean score seconds in `score/` and
  `clean/`, piece seconds in `timed/` and track seconds in the interchange
  (new-opera-model.md A.1, "Time bases"). Record the clock in each stage's state record, or
  rename the fields.
- [ ] **Reminder, with format 1.2: nothing reads `score_start`/`score_end`.** No roon-rd code
  uses them (checked 2026-11-18), and route C segments lack them. Decide whether the
  interchange keeps them (e.g. for a score-following view) or drops them.
- [ ] **Reminder, with P4: no file records what produced it.** Only the interchange carries
  a version; no stage output records its inputs or the tool version. P4's state records
  are the fix; check they cover every format in new-opera-model.md A.2-A.11.
- [ ] **D7: gloss in the tool.** Discuss moving the verse list, phrase split and gloss from
  the session into the tool (Claude API).
- [ ] **D21 / D24: before any re-time of all pieces.** A full re-time would cut Solti's No. 9
  short (D21) and drop No. 24's kept end (D24).
- [ ] Then Così per repertoire-plan.md: "nothing new", so it measures what a second opera
  costs.

## Later

- [ ] **Google Drive quota for `libretto-work`** (D27): it now syncs to Google Drive. Figaro
  is 2.8 GB, mostly stems; the whole repertoire was projected at 80-240 GB of stems. Check
  the Drive plan before Wagner, or keep stems in a folder outside My Drive (a small
  `work_root` split).

## Done

- 2026-11-18: uv cache moved out of Drive to `/Volumes/JDR Desktop Attached/libretto-cache/uv`
  (1.3 GB, packages kept; D27).
- 2026-11-18: `libretto-work` moved to Google Drive on the external disk; config, settings
  and docs updated (D27). Replaces "Where `libretto-work` lives".
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
