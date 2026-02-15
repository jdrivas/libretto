# Murashev.com Investigation Findings

**Date:** 2025-02-14
**Investigated:** `www.murashev.com/opera/Le_nozze_di_Figaro_libretto_English_Italian_Act_1`
**Purpose:** Determine how murashev.com serves libretto content and the best approach for automated extraction.

---

## Key Finding: NOT a SPA

**Our earlier assessment was wrong.** Murashev.com is **not** a JavaScript SPA. It is classic server-rendered HTML. The full libretto content is present in the initial HTTP response — no API calls, no AJAX, no dynamic content loading.

The earlier `read_url_content` attempts returned only navigation elements because that tool's text extraction couldn't properly parse the HTML table structure. But `curl` + HTML parsing confirms all content is in the page source.

### Technology Stack

- **jQuery 1.7.1** — used only for show/hide toggles on navigation sections
- **Dreamweaver templates** — `InstanceBegin`/`InstanceEnd` markers in the HTML
- **Static HTML** — standard server-rendered pages, no SPA framework
- **No robots.txt** — returns 404

---

## Side-by-Side Page Structure

### URL Pattern

```
https://www.murashev.com/opera/{Opera}_libretto_{Lang1}_{Lang2}
https://www.murashev.com/opera/{Opera}_libretto_{Lang1}_{Lang2}_Act_{N}
```

Examples for Le nozze di Figaro:
- `Le_nozze_di_Figaro_libretto_English_Italian` (or `_Act_1` through `_Act_4`)
- `Le_nozze_di_Figaro_libretto_Italian_English` (swapped column order)
- `Le_nozze_di_Figaro_libretto_Italian_German`
- `Le_nozze_di_Figaro_libretto_Italian_French`

**Note:** The `_Act_N` suffix pages still return the **entire opera** (all 4 acts, 140 rows). The act suffix appears to be for navigation/anchoring only — it does not filter content. We can use any of these URLs and get the complete opera.

### HTML Table Layout

The libretto is in a two-column HTML table:

```html
<table width="100%" border="0" cellspacing="1" cellpadding="5">
  <tr>
    <td width="50%" valign="top"><!-- Language 1 (English) --></td>
    <td width="50%" valign="top"><!-- Language 2 (Italian) --></td>
  </tr>
  <!-- ... 140 rows total ... -->
</table>
```

Each `<tr>` is a **paragraph-aligned segment pair**. The two cells contain the same passage in each language, with identical structural markers. This is exactly the pre-aligned bilingual data we need.

### Content Statistics (Le nozze di Figaro)

| Metric | Value |
|--------|-------|
| Total rows | 140 |
| Columns per row | 2 |
| Page size | ~256 KB |
| Acts | 4 (all in one page) |
| Musical numbers | 28 + Overture |

---

## HTML Markup Conventions

### Act/Section Headers

```html
<span class="act"><act>ACT ONE</act><br />
</span>
```

Italian equivalent:
```html
<span class="act"><act>ATTO PRIMO</act><br />
</span>
```

Custom `<act>` tag is used (not a standard HTML element). CSS: `font-size: 1.2em; font-weight: bold`.

### Musical Number Labels

```html
<b>No. 1: Duettino</b><br />
```

Italian equivalent:
```html
<b>N° 1: Duettino</b><br />
```

Pattern: English uses `No. N: Type`, Italian uses `N° N: Type`.

### Character Names

Plain text in ALL CAPS, followed by `<br />`:

```html
FIGARO<br />
```

Ensemble characters joined with comma:
```html
SUSANNA, FIGARO<br />
```

### Stage Directions

Wrapped in `<i>` tags:

```html
<i>(A half-furnished room with a large armchair in the<br />
centre. Figaro is measuring the floor.)</i><br />
```

Also appear inline:
```html
MARCELLINA (making a curtsy)<br />
```

### Sung/Spoken Text

Plain text with `<br />` line breaks. Blank lines (double `<br />`) typically separate stanzas or character turns.

### Repeat Markers

```html
If you would dance, <i>etc.</i><br />
```

Italian:
```html
Se vuol ballare, <i>ecc.</i><br />
```

---

## Row-to-Musical-Number Mapping (Le nozze di Figaro)

The 140 rows contain the following structural segments:

| Row | EN Marker | IT Marker |
|-----|-----------|-----------|
| 0 | (empty) | (empty) |
| 1 | Cast / Overture | Personaggi / Sinfonia |
| 2 | **ACT ONE** / No. 1: Duettino | ATTO PRIMO / N° 1: Duettino |
| 5 | No. 2: Duettino | N° 2: Duettino |
| 10 | No. 3: Cavatina | N° 3: Cavatina |
| 11 | No. 4: Aria | N° 4: Aria |
| 13 | No. 5: Duettino | N° 5: Duettino |
| 17 | No. 6: Aria | N° 6: Aria |
| 24 | No. 7: Terzetto | N° 7: Terzetto |
| 29 | No. 8: Chorus | N° 8: Coro |
| 33 | No. 9: Aria | N° 9: Aria |
| 35 | **ACT TWO** / No. 10: Cavatina | ATTO SECONDO / N° 10: Cavatina |
| 41 | No. 11: Song | N° 11: Canzone |
| 44 | No. 12: Aria | N° 12: Aria |
| 51 | No. 13: Terzetto | N° 13: Terzetto |
| 55 | No. 14: Duettino | N° 14: Duettino |
| 59 | No. 15: Finale | N° 15: Finale |
| 82 | **ACT THREE** | ATTO TERZO |
| 85 | No. 16: Duet | N° 16: Duetto |
| 90 | No. 17: Recitative and Aria | N° 17: Recitativo ed Aria |
| 95 | No. 18: Sextet | N° 18: Sestetto |
| 101 | No. 19: Recitative and Aria | N° 19: Recitativo ed Aria |
| 103 | No. 20: Duettino | N° 20: Duettino |
| 105 | No. 21: Chorus | N° 21: Coro |
| 111 | No. 22: Finale | N° 22: Finale |
| 114 | **ACT FOUR** / No. 23: Cavatina | ATTO QUARTO / N° 23: Cavatina |
| 122 | No. 26: Recitative and Aria | N° 26: Recitativo ed Aria |
| 125 | No. 27: Recitative and Aria | N° 27: Recitativo ed Aria |
| 126 | No. 28: Finale | N° 28: Finale |
| 139 | End of the Opera | Fin dell'opera |

**Note:** Numbers 24 and 25 appear to be missing from this source (these are sometimes omitted in certain editions — Nos. 24-25 are Marcellina's and Basilio's arias that are frequently cut in performance).

---

## Single-Language Page Structure

Single-language pages use a different layout:

```
URL: /opera/Le_nozze_di_Figaro_libretto_Italian_Act_1
```

```html
<table width="80%" border="0" cellspacing="1" cellpadding="5">
  <tr>
    <td valign="top"><!-- Single language content --></td>
  </tr>
  <!-- 38 rows -->
</table>
```

- **1 column** instead of 2
- **`width="80%"`** instead of `100%`
- **38 rows** (vs 140 for side-by-side) — single-language pages appear to split by act
- Same HTML markup conventions (`<act>`, `<b>`, `<i>`, `<br />`)

The side-by-side pages are more useful for our purposes since they give us pre-aligned bilingual text.

---

## Scraping Assessment

### Difficulty: EASY

This is as straightforward as opera-arias.com. Standard HTTP GET + HTML parsing.

### Rust Implementation

```
reqwest   → fetch the HTML page
scraper   → parse HTML, select the libretto table with CSS selectors
```

No headless browser needed. No JavaScript execution required. No API to reverse-engineer.

### CSS Selectors for Extraction

```css
/* The libretto table (side-by-side pages) */
table[width="100%"][border="0"][cellspacing="1"][cellpadding="5"]

/* Individual rows */
table[width="100%"] tr

/* Language cells */
table[width="100%"] td[width="50%"]

/* Act/section headers */
span.act act

/* Musical number labels */
td b

/* Stage directions */
td i
```

### Extraction Algorithm

1. Fetch HTML with `reqwest`.
2. Parse with `scraper`.
3. Select the libretto table: `table[width="100%"][border="0"][cellspacing="1"]`.
4. Iterate over `<tr>` rows.
5. For each row, extract the two `<td>` cells → Language 1 (EN) and Language 2 (IT).
6. Within each cell:
   - Detect `<act>` tags → act/section boundaries.
   - Detect `<b>` tags → musical number labels.
   - Detect `<i>` tags → stage directions.
   - All other text → character names and sung text.
   - Split on `<br />` for line-level granularity.
7. Output paired (EN, IT) paragraphs with structural annotations.

### Rate Limiting

One request fetches the entire opera (all acts). For Le nozze di Figaro, we need exactly **1 HTTP request** for the side-by-side bilingual text. Rate limiting is barely a concern.

---

## Comparison with opera-arias.com

| Aspect | opera-arias.com | murashev.com |
|--------|----------------|---------------|
| **Rendering** | Server-side | Server-side |
| **Content in HTML** | Yes | Yes |
| **JS required** | No | No |
| **Scraping** | `reqwest` + `scraper` | `reqwest` + `scraper` |
| **Languages per page** | 1 | 2 (side-by-side) |
| **Alignment** | Must align post-hoc | Pre-aligned in table rows |
| **Musical number labels** | `No. N - Type` | `No. N: Type` / `N° N: Type` |
| **Requests per opera** | 2 (IT + EN pages) | 1 (side-by-side page) |
| **Unique value** | Broader coverage, cleaner text | Pre-aligned bilingual pairs |

---

## Recommendation

**Murashev.com should be the primary source for bilingual acquisition.** The pre-aligned table rows give us paragraph-level EN/IT pairing with zero alignment work. One HTTP request per opera.

Use opera-arias.com as a secondary source for:
- Cross-referencing text accuracy.
- Operas not available on murashev.com.
- Additional language versions.

### Impact on Development Plan

The scraping difficulty for murashev.com has been downgraded from **Medium-Hard** to **Easy**. The same Rust crates (`reqwest` + `scraper`) work for both sites. No headless browser (`chromiumoxide`) or API reverse-engineering needed.

This significantly simplifies Phase 2 (Acquisition Tool) of the development plan.
