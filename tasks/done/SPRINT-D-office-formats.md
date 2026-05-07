# Sprint D — Office Document Conversion

**Goal:** Move office formats from preview-only to actual conversion output. Tool chain: LibreOffice headless (primary) + pandoc (text-oriented formats).

**Entry condition:** `convert/document.rs` pipeline stable. LibreOffice available in dev environment.

---

## TASK-D1: LibreOffice headless conversion pipeline

**Scope:** Core infrastructure for all office format conversions.

**What to do:**
- Add `libreoffice_convert(input, output_format, output_path)` in `convert/document.rs`
- Command: `libreoffice --headless --convert-to <fmt> --outdir <dir> <input>`
- Detect LibreOffice binary: try `libreoffice`, `soffice`, macOS app bundle paths (`/Applications/LibreOffice.app/Contents/MacOS/soffice`)
- If missing: emit clear error "Office conversion requires LibreOffice"
- Handle LibreOffice's non-standard stdout (it logs to stderr and writes a fixed-name output file; rename to expected output path)
- Add unit tests for binary detection

**Done when:** `libreoffice_convert` calls succeed for a simple DOCX → PDF case. CI green.

---

## TASK-D2: Word document conversion (DOCX, DOC, RTF, ODT)

**Scope:** Enable DOCX/DOC/RTF/ODT as live input and output formats.

**What to do:**
- Wire DOCX → PDF, HTML, TXT, ODT via LibreOffice headless
- Wire DOC → same targets (LibreOffice handles DOC natively)
- Wire RTF → PDF, DOCX, ODT
- Wire ODT → PDF, DOCX, RTF
- Set all 4 formats to `live: true` in document picker
- Add sweep cases for each conversion pair
- pandoc handles DOCX ↔ MD, DOCX ↔ HTML well — use pandoc for those paths, LibreOffice for PDF

**Done when:** All word format conversions have sweep coverage and pass. CI green.

---

## TASK-D3: Spreadsheet conversion (XLSX, XLS, ODS)

**Scope:** Enable spreadsheet formats.

**What to do:**
- XLSX/XLS → PDF, ODS, CSV via LibreOffice headless
- ODS → XLSX, PDF, CSV via LibreOffice headless
- For CSV output: LibreOffice exports first sheet only — document this behavior
- Set formats to `live: true`
- Add sweep cases

**Done when:** Spreadsheet conversion sweep passes. CI green.

---

## TASK-D4: Presentation conversion (PPTX, PPT, ODP)

**Scope:** Enable presentation formats.

**What to do:**
- PPTX/PPT → PDF, ODP via LibreOffice headless
- ODP → PPTX, PDF
- Image output (each slide as PNG): LibreOffice supports `--convert-to png` for presentations — wire as optional output
- Set formats to `live: true`
- Add sweep cases

**Done when:** Presentation conversion sweep passes. CI green.

---

## TASK-D5: Apple iWork formats (Keynote, Pages, Numbers)

**Scope:** Enable Pages, Keynote, Numbers as input formats.

**What to do:**
- LibreOffice can open Pages/Numbers/Keynote files (with limitations)
- macOS only: `textutil` can convert Pages → TXT/HTML/RTF
- Wire: Pages → PDF/DOCX (LibreOffice or textutil), Numbers → XLSX/CSV, Keynote → PDF/PPTX
- If LibreOffice conversion fails silently (common with iWork): detect empty output and emit error
- Document macOS-only limitation clearly in UI tooltip

**Done when:** iWork formats convert on macOS or emit clear platform error. CI green.

---

## TASK-D6: MSG email conversion

**Scope:** Enable MSG (Outlook) as a live input format (currently `todo: true`, preview only).

**What to do:**
- `convert/email.rs` already has EML/MBOX support
- MSG requires `msgconvert` (Perl, part of libemail-outlook-message-perl) or `libpst`
- Add MSG → EML conversion: `msgconvert --outdir <dir> <input.msg>`
- Detect binary; emit clear error if missing
- Set `msg` to `live: true`
- Add sweep case

**Done when:** MSG → EML conversion passes or emits dependency error. CI green.
