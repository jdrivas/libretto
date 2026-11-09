# Repertoire plan: which operas, in what order

*Agreed 2026-11-08. The target repertoire is:*
- *all three Mozart/Da Ponte operas;*
- *most of Wagner, including a full Ring, probably starting with Solti;*
- *Strauss.*

*The pipeline itself is described in `sung-text-pipeline.md`.*

## Principle

Each opera should bring in roughly one new difficulty. After each new difficulty, build one
opera of the same kind. That makes the tools solid, and it measures what a familiar kind of
opera now costs. Where two recordings are available, build both: the second recording is what
proves that adding a recording needs no extra work.

## Order

**Mozart and Da Ponte** (Bärenreiter vocal scores, born-digital)

| # | Opera | What it adds | Why here |
|---|---|---|---|
| 1 | Le nozze di Figaro | builds the tools | in progress |
| 2 | Così fan tutte | nothing new | cleanest measure of a second opera's cost: same publisher, one version, ensemble-rich |
| 3 | Don Giovanni | versions (Prague and Vienna), optional arias | forces detecting which numbers a recording contains |

**Wagner** (Schott vocal scores)

| # | Opera | What it adds | Why here |
|---|---|---|---|
| 4 | Das Rheingold (Solti) | German, Schott score, a 2½-hour continuous act | one bundle of new things, but no chorus and few ensembles; the start of the Ring |
| 4b | Das Rheingold, second recording | nothing new | confirms the German path needs no per-recording work |
| 5 | Die Walküre | Valkyries' octet | mostly scenes for two singers; consolidates long acts |
| 6 | Siegfried | nothing new | the easiest Ring opera for us: almost no ensembles |
| 7 | Götterdämmerung | chorus (the vassals), trios (Norns, Rhinemaidens) | first real chorus, in a style already handled |
| 8 | Tristan und Isolde | nothing new | should be close to automatic by then |
| 9 | Lohengrin, Der fliegende Holländer, Tannhäuser | heavy chorus; versions (Tannhäuser Dresden/Paris, Holländer in one act or three) | reuses the version handling from Don Giovanni |
| 10 | Die Meistersinger, Parsifal | densest Wagner ensembles (quintet, riot scene, Flower Maidens) | hardest Wagner, when the tools are mature |

**Strauss**

| # | Opera | What it adds | Why here |
|---|---|---|---|
| 11 | Salome | Strauss's orchestra and publisher; one act of about 100 min | mostly solo singing, so the harder vocal separation is the main new problem |
| 12 | Elektra | heaviest orchestra | worst case for the vocal stem, in a known form |
| 13 | Der Rosenkavalier | dense conversational ensembles, the trio | hardest ensemble test |
| 14 | Ariadne auf Naxos, Arabella, Die Frau ohne Schatten, Capriccio | versions (Ariadne 1912/1916), Capriccio's octet | variations on solved problems |

## Checkpoints

Take stock, as in step 5 of the Sextet plan:
- after Così;
- after the second Rheingold recording;
- after Rosenkavalier.

At each one, use the measured cost per opera to decide whether to go on or to restructure the
tools first.

## Scores

- **Mozart:** Bärenreiter vocal scores, as born-digital PDFs. Check each new PDF's fonts
  before relying on it: Bärenreiter scores were engraved over decades and may use different
  software.
- **Wagner and the German operas:** Schott's Urtext piano-vocal reductions. Per Gemini, these
  are sold as E-Score PDFs that are native vector files. This is not verified yet; the user
  will buy one to verify when we get close.
- **Full scores:** Eulenburg miniature full scores are also available as vector PDFs. They
  are not needed for the pipeline (see `sung-text-pipeline.md`). They may be useful later for
  score-informed vocal separation, or for showing bar and page references in the edition
  being read along with.
- **Reading along:** use the vocal score we decode, so the bar locations shown match the page
  exactly.
- **Licensing:** score PDFs and anything decoded from them are never shared. Interchange
  files (timings, sung text, glosses) may be shared.
