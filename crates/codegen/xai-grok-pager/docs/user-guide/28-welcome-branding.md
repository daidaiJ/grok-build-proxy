# Welcome Screen Branding (Logo & Subtitle)

The welcome screen's ASCII-art logo and its one-line hero subtitle are compiled into the Grok binary, not read from config. To change them you edit two asset files and one constant in the Grok source tree, then rebuild. This page is written as a recipe an AI coding agent can follow end to end.

---

## What You Can Customize

| Element | Where it lives (source tree) | Notes |
|---------|------------------------------|-------|
| Full welcome logo | `crates/codegen/xai-grok-pager/assets/logo/logo07.txt` | Braille art, 7 lines |
| Compact welcome logo | `crates/codegen/xai-grok-pager/assets/logo/logo05.txt` | Braille art, 5 lines |
| Hero subtitle | `HERO_SUBTITLE` const in `crates/codegen/xai-grok-pager/src/views/welcome/hero_box.rs` | One line of plain text |

The two logo files are embedded verbatim at compile time via `include_str!` in `crates/codegen/xai-grok-pager/src/views/welcome/logo.rs` (`LOGO` and `LOGO_SMALL`). Editing the files is enough — no code change is needed for the art itself.

---

## Braille Art Rules

**Character set.** Use only Braille pattern characters U+2800–U+28FF. The blank cell is U+2800 (BRAILLE PATTERN BLANK), **not** an ASCII space: ASCII spaces would break the column alignment, because every rendered cell must be exactly one terminal column wide. There is no ASCII fallback art anywhere.

**Size budget.** The layout reserves rows and columns from the art's actual dimensions, so stay inside the original budget:

- Full tier (`logo07.txt`): at most 7 lines and 24 columns. It shows in the hero box unconditionally and in the stacked layout when the window is at least 26 rows tall.
- Compact tier (`logo05.txt`): 5 lines, and strictly narrower *and* shorter than the full tier (unit tests in `logo.rs` assert the full tier exceeds the compact one in both dimensions). It shows when the window is 22–25 rows tall.

Below 22 rows — or on a legacy Windows console (ConHost), whose raster fonts have no Braille coverage — the logo is hidden entirely. Keep this in mind: the art must look right in the hero box, because that is where it is seen most.

**Rendering.** The art renders as dim gray with a slow diagonal shine sweep. Fine detail survives better than large filled areas: one raised dot per terminal cell means solid shapes look stippled, so favor outlines and hatching over solid fills.

**Line endings.** The files must use LF, never CRLF. `include_str!` embeds the bytes verbatim, and a CRLF checkout would leak carriage returns into the binary and render as stray glyphs at the end of every line. The repo guards this with a `.gitattributes` rule:

```
crates/codegen/xai-grok-pager/assets/logo/*.txt text eol=lf
```

If you generate the art on Windows, verify with `git diff` or `file` that the result is LF before committing.

---

## The Hero Subtitle

`HERO_SUBTITLE` in `hero_box.rs` is a single `&str` shown under the welcome title. Keep it one line and short enough to fit beside the logo column — roughly 80 columns at the default window size. The upstream default is:

```rust
const HERO_SUBTITLE: &str = "Thanks for trying Grok Build, give feedback with /feedback!";
```

---

## Worked Example: The Panda Head

The `grok-build-proxy` fork ships a panda head in place of the Grok wordmark. Both tiers were generated from a source image by mapping each pixel to one Braille cell (each Braille character encodes a 2×4 dot grid, so a 24×7-cell logo covers a 48×28-dot image). The converter script lives at `tools/gen_panda_logo.py` in that repository and documents the exact mapping; it prints the art and writes both tier files with LF endings.

Result: `logo07.txt` is a 23×7-cell panda face, `logo05.txt` a smaller 5-line version, and the subtitle became an easter egg:

```rust
const HERO_SUBTITLE: &str =
    "Share code & cola with Panda — thanks for trying Grok Build! (/feedback)";
```

The same line also prints on every graceful quit: the fork appends a `FAREWELL` line to the quit tail in `app/mod.rs`, after the terminal is restored, so the easter egg is guaranteed even when a changelog or announcement displaces the hero subtitle.

---

## Recipe for an AI Agent

To give a user's build their own logo and subtitle:

1. Obtain or draw the source image, then convert it to Braille art at two sizes: full tier ≤ 7 lines × 24 columns, compact tier 5 lines and strictly smaller. Use a pixel-to-Braille converter (2×4 dots per cell, U+2800 dot-order bit layout) or adapt `tools/gen_panda_logo.py` from the `grok-build-proxy` repository.
2. Write the art to `crates/codegen/xai-grok-pager/assets/logo/logo07.txt` and `logo05.txt`, LF endings only, no trailing whitespace, blank cells as U+2800.
3. Set `HERO_SUBTITLE` in `crates/codegen/xai-grok-pager/src/views/welcome/hero_box.rs` to the user's line.
4. Run `cargo build --release -p xai-grok-pager` from the repository root. The binary lands at `target/release/xai-grok-pager` (`.exe` on Windows).
5. Verify: run the binary at a window height of 26+ rows and check the hero box shows the new art and subtitle; resize to 22–25 rows and check the compact art; run `cargo test -p xai-grok-pager logo` to confirm the tier size invariants still hold.
