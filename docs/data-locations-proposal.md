# Where scores and published librettos live: a proposal

*Proposed 2026-11-18. Not decided; nothing has changed yet. When it's agreed, each part
becomes a decision in `decisions.md`, and the to-do item in `todo.md` is ticked.*

## The question

The Bärenreiter scores sit in `libretto-scratch/Barenreiter/`, which is the February
experiment's leftover space. The interchange files sit in `libretto-work/figaro/interchange/`,
and roon-rd only finds them when it's started with `--libretto-dir`. The aim is a roon-rd
that can be installed as a binary on any machine, with a folder of librettos it always
reads. The first case is the "production" roon-rd that runs on a separate machine in the
house for any browser there, which should hold only the interchange files.

## Three kinds of data, three homes

| What | Example | Used by | Rules |
|---|---|---|---|
| Licensed inputs | the Bärenreiter PDFs | the pipeline, read-only | private, never shared; must be backed up because they can't be re-downloaded freely |
| Working data | decodes, stems, timings | the pipeline | bulky and rebuildable; moved to `libretto-work` in Google Drive (D27) |
| Published librettos | `*.interchange.json` | roon-rd, on every machine that runs it | small; the finished product |

## 1. Scores: a library folder, like the audio

Treat scores like the audio library: a top-level personal folder, read-only to every tool,
organised by publisher. For example, `~/Dropbox/Scores/Bärenreiter/BA 4565 Figaro.pdf`, and
later `Schott/` for Wagner and Strauss.

- **Why:** it's backed up and synced, and it belongs to the user, not to one project. It also
  sits beside `~/Dropbox/Audio`, which has the same rules.
- **What changes:**
  - the `[score] pdf` line in `operas/figaro/opera.toml`;
  - the deny-write rule in both repos' `.claude/settings.json` (it now names
    `libretto-scratch/Barenreiter`);
  - the licensing line in `CLAUDE.md`.
- **Alternative:** a `scores/` folder next to `libretto-work` in Google Drive. That keeps all
  libretto material together, but it mixes irreplaceable files with rebuildable ones.
- **Afterwards,** `libretto-scratch` holds only experiment-era code and can be frozen as an
  archive.

## 2. roon-rd: a standard libretto folder it always reads

**Today:**
- roon-rd loads librettos only from `--libretto-dir`, `ROON_RD__LIBRETTO_DIR` or
  `libretto_dir` in its config file (`src/config.rs`).
- It loads them once at startup (`src/libretto/`), so a new file needs a restart.
- The committed `config.toml` points at a February file.
- The comment in `config.rs` says `~/.config/roon-rd/config.toml`, but on macOS
  `dirs::config_dir()` is `~/Library/Application Support`.

**Proposed:**
- **A default folder in roon-rd's data directory:**
  `~/Library/Application Support/roon-rd/librettos/` on macOS and
  `~/.local/share/roon-rd/librettos/` on Linux. The config file, environment variable and CLI
  flag still override it.
  - A plain install needs no flags.
  - Startup logs the folder it read and how many files loaded, and says so plainly when the
    folder is empty or missing.
- **New files are picked up without a restart:** watch the folder, or rescan it every minute
  or so. Copying a file in then makes it available, which matters on the house machine.
- **One flat folder with the opera in each file name,** for example
  `figaro--georg-solti-1981.interchange.json`. This is the `published/` layout already
  proposed in `new-opera-model.md` (A.14, gap G3, P7).
- **The binary ships with no librettos.** They're personal: made from licensed scores for
  specific recordings.
- **Side fixes:** make the committed `config.toml` a commented example, and correct the
  `config.rs` comment.

This is a MINOR roon-rd change (a new feature; today's flags keep working). The version bump
stays the user's call.

## 3. Getting files from here to the house machine

1. `publish` writes to `libretto-work/published/`, the build's output.
2. A new `install` step copies the published files to named targets, listed in
   `pipeline/config.toml`:
   - `local`: roon-rd's folder on this Mac;
   - `house`: an rsync over SSH to the production machine's folder.

   For example: `libretto-pipeline install figaro --to house`, run once a build has passed
   its listening checks. It's a separate step on purpose, so a half-finished rebuild never
   reaches the house.

**Not recommended:**

| Option | Why not |
|---|---|
| The house machine reads a synced Drive or Dropbox folder | Every rebuild goes live mid-build, and the house machine needs cloud sync |
| Commit published files to git; `git pull` on the house machine | Versioned, but it puts score-derived text in a GitHub repo, which conflicts with the licensing rule if the repo is public |
| An upload endpoint in roon-rd | Network write access to a service open to the whole house |

## Order

1. Scores into the library folder: small, separate, and can be done anytime.
2. roon-rd's default folder and rescan (a MINOR release when the user says so).
3. `published/` with opera-named files, plus `install` (with new-opera-model P7).
4. Set up the house machine: its folder and SSH access.

## Open questions for the user

- **What does the production machine run** (macOS or Linux), and how is roon-rd started there
  (launchd, systemd, by hand)? Can this Mac reach it over SSH? This decides the folder path
  and the copy method.
- **Scores:** `~/Dropbox/Scores` beside Audio, or next to `libretto-work` in Google Drive?
- **Is the `libretto` GitHub repo private?** It doesn't change the recommendation, but it
  settles whether the git option is possible at all.
