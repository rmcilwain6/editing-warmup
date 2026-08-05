# TestStrip — Roadmap to Shareable MVP

Status as of this doc: functional POC. Session flow, folder pickers, and a
Settings panel (photo count, timer, challenge list with custom additions,
disk-persisted) all work on Windows in dev mode. Nothing has been packaged
or shared yet. This document lays out what's left to get from "works on my
machine in dev mode" to "something I can hand to a few people and get
feedback on."

## Current state (what exists today)

- Tauri v2 + Rust backend, vanilla HTML/JS/CSS frontend (no bundler/build step)
- Session flow: pick N random `.CR2` files from an archive folder, open each
  in the OS-default handler for that extension, watch an export folder for
  the matching JPG, per-photo countdown timer, summary screen
- Folder pickers for archive root / export root (native dialogs)
- Settings panel: photos-per-session, seconds-per-photo, a flat list of
  editing "challenges" (enable/disable + add custom ones), persisted to a
  JSON file in the OS app-data directory
- Windows-only so far; not packaged as an installer; icon is `.ico` only

## Open product questions (worth answering before or alongside building)

- [ ] Who is this actually for — just you, or friends/other photographers?
      That changes how much polish/robustness is worth investing before v1.
- [ ] Is Lightroom-specific, or editor-agnostic, the right default framing?
      (See "Editor selection" below — currently it's neither; it just opens
      whatever the OS has set as default for `.CR2`.)
- [ ] Do you want telemetry/feedback collection at all (e.g. "how was this
      session"), or is feedback purely informal (Slack/text/in person)?

## Feature backlog to MVP

### 1. Editor selection (currently hardcoded to "OS default handler")
- [ ] Settings option: "Use system default" (current behavior, zero setup)
      vs. "Choose a specific program" (browse to an `.exe`/`.app`, store the
      path, launch via `Command::new(path).arg(raw_path)` instead of `open::that`)
- [ ] Nice-to-have, not required for v1: auto-detect common editors (Lightroom,
      Capture One, darktable, RawTherapee) via Windows registry "App Paths"
      keys / macOS `/Applications` scan, offered as quick-pick buttons
- [ ] In-app first-run instructions for setting the OS default handler, so
      users don't have to read the README to get started

### 2. Robustness / error handling polish
- [ ] Archive folder with zero `.CR2` files: currently emits a generic error;
      make this friendlier and non-blocking (offer to reselect folder)
- [ ] Export folder on a flaky/network drive: the file watcher assumes a
      local, reliable filesystem — test and harden
- [ ] What happens if the user closes Lightroom/the editor mid-session, or
      never exports? (Currently: timer expires, user can skip/keep working —
      probably fine, but worth explicitly deciding this is the intended UX)
- [ ] "Restore defaults" button in Settings (reset challenges/timer/count)
- [x] Remember last-used archive/export folders across restarts (persisted
      in settings.json, prefilled on launch)
- [ ] Frontend source layout: `dist/` is hand-authored HTML/JS/CSS being used
      directly as source (no bundler/build step), but the name implies a
      generated build artifact. Rename to something intentional (e.g. `web/`
      or `ui/`) and update `frontendDist` in `tauri.conf.json` accordingly —
      purely a naming/organization cleanup, no behavior change

### 3. Reject & redraw (distinct from skip) — done
Today's "Skip" and "Next" buttons do the same thing under the hood
(`skip_step`/`manual_next` both just call `advance_step`, which always moves
to the next slot in the session) — there was no way to reject the *current*
photo without spending one of the N session slots on it. Problem: the random
picker occasionally surfaces a photo that can't be professionally
edited/shared in this context (client work, a person who hasn't consented to
sharing, etc.).
- [x] "Can't use this photo" button added to both the active and expired
      panels, separate from Skip/Next
- [x] New `reject_current` Rust command: redraws a single replacement photo
      via `random_walk_pick` (excluding paths already in the session's
      `picks`), swaps it into the current slot, and re-invokes `start_step`
      for that same slot — session length (N) is preserved
- [x] Fixed a generation-counter bug this surfaced: the old timer-thread
      staleness check compared against `current_idx`, which reject leaves
      unchanged after redraw (decrement then re-increment lands on the same
      value) — two timers would have run concurrently for the same slot.
      Added a `step_generation` counter to `SessionState`, bumped every time
      `start_step` actually starts a step, and use that for the staleness
      check instead
- [ ] Not yet addressed: UX for any `.xmp`/partial edit state left behind on
      the rejected RAW (only relevant once seeded-start below exists)
- [ ] Not yet addressed: a per-session "rejected" count/log so repeated
      reject-and-redraw can't loop forever if the archive is small or mostly
      unshareable — currently `pick_replacement` just returns `None` after
      `ATTEMPTS_PER_PHOTO` failed tries, surfaced as an error in the UI

### 4. Per-photo prompt manifest / output view
Problem: once a session ends, there's no record of which challenge/prompt
was attached to which photo. `session_finished` (src-tauri/src/main.rs) only
emits a list of exported filenames — the challenge text picked per step
(`StepPayload.challenge`) isn't retained anywhere past that single event. For
posting later, you need to know "what was I told to do with this one" days
after the session happened.
Three options, not mutually exclusive:
- [ ] **Enhanced summary screen**: track `(raw_path, expected_jpg, challenge)`
      per step in `SessionState` as the session runs (currently only
      `picks: Vec<PathBuf>` — needs a parallel/paired vec or a small struct),
      and show challenge text next to each exported file on the existing
      summary panel instead of just filenames
- [ ] **Sidecar/manifest file alongside the export**: write either a
      `<jpgname>.txt` per photo or a single `manifest.json`/`.csv` in the
      session's export folder (`create_session_dir` already makes this
      per-session folder) listing photo → challenge — travels with the files
      if you copy them out for posting later
- [ ] **Cross-session history log**: append each session's photo/challenge pairs
      to a persistent log (e.g. JSONL in the app-data dir, alongside
      `settings.json`) so past sessions remain searchable later, not just
      the one just-finished summary screen
- [ ] Recommended starting point: do the summary-screen version first (small,
      contained to state already flowing through the app), then add the
      sidecar manifest since it's the one that actually solves "I need this
      information sitting next to the file when I go to post it"

### 5. Stats over time
Builds directly on the cross-session history log from #4 above — once every
session is logged persistently (photo, challenge, outcome, timestamps), the
same data supports a stats/progress view:
- [ ] Completion rate: fraction of photos exported before the timer expired
      vs. after expiry vs. never exported (currently the app doesn't
      distinguish these outcomes at all — `time_expired`/`export_detected`
      are transient UI events, nothing records which one "won" for a step)
- [ ] Skipped vs. delivered counts, overall and per challenge/prompt, to
      surface which challenges tend to get abandoned
- [ ] Source breakdown: which archive folders/subfolders exported photos
      came from (useful once the archive covers multiple shoots/years —
      `random_walk_pick` already knows the picked path, just isn't retained)
- [ ] A simple stats screen/panel reading the history log and computing these
      aggregates on demand — no need for a database at personal-use scale,
      the JSONL log can just be scanned and summarized in Rust or JS
- [ ] Sequencing note: this is a consumer of #4's history log, not a
      standalone feature — implement the persistent log first, stats view
      second

### 6. Seeded-start challenges (XMP sidecars)
Idea: before opening a RAW, write a `.xmp` sidecar next to it with specific
Camera Raw develop settings already applied (e.g. forced grayscale, wrecked
white balance, crushed exposure, a hard crop). The editor opens to that
seeded state instead of untouched RAW data, and the challenge becomes
"here's a photo with specific changes already made — don't undo them, edit
from here." This is a new challenge *type* alongside today's flat text
prompts, not a replacement for them.
- [ ] Define a `SeededChallenge` variant (vs. today's plain-text `ChallengeDef`)
      with a set of `crs:` fields to write (grayscale, WB/temp/tint, exposure,
      crop, etc.)
- [ ] Rust XMP sidecar writer: minimal RDF/XML template with the Camera Raw
      namespace, filled from the seeded challenge's field values
- [ ] Write the sidecar next to the picked RAW immediately before `open::that`
      in `start_step()` (src-tauri/src/main.rs)
- [ ] Settings UI: author/enable seeded challenges (which fields + values),
      similar to the existing custom-challenge flow
- [ ] Known limitation to design around: this only works reliably the *first*
      time a RAW is opened/imported into a Lightroom catalog. If the archive
      photo has already been cataloged before, Lightroom won't pick up a
      sidecar dropped next to it without a manual "Read Metadata from File" —
      confirm the archive-folder workflow guarantees fresh, never-imported
      files before relying on this
- [ ] Not in scope: true constraint *enforcement* (blocking specific sliders,
      preventing reset) — would require a Lightroom Lua plugin and abandons
      the "any OS-default editor" architecture. This feature is a seeded
      starting point only, not a guardrail.

### 7. Challenge content
- [ ] Revisit the starter challenge list — the first draft is a placeholder;
      worth a pass once a few real sessions have been run
- [ ] Decide if some sessions should force at least one challenge from a
      given "family" (e.g. always at least one b&w challenge per week) —
      explicitly deferred earlier when "flat list, no categories" was chosen;
      revisit if variety turns out to be a problem in practice

### 8. Visual design
Right now the UI is unstyled functional HTML (system font, default blue
buttons, no real visual identity). To actually be "designed":
- [ ] Decide on a visual direction (this is a small always-on-top utility
      window — should it look like a tool, a game, something else?)
- [ ] Real typography/spacing/color pass, likely dark-mode-aware
- [ ] Consider whether the fixed-size window model still holds once Settings
      has grown (it's already resizable now — decide if that's final)
- [ ] Icon: currently only a placeholder `.ico`; needs a real app icon in
      all required formats per platform (see Packaging below)

### 9. Audible cues
Small UX addition: sound effects for key session moments (step start, export
detected/success, timer expiring, session finished), with a Settings toggle
to mute them entirely.
- [ ] Pick/produce a small set of short sound assets (start, success, warning,
      finish) — a few seconds each, license-clear or self-made
- [ ] Bundle assets as Tauri resources and play them from the frontend
      (`dist/main.js`) on the relevant `listen(...)` events (`step_started`,
      `export_detected`, `time_expired`, `session_finished`), using the Web
      Audio API/`<audio>` — no new Rust dependency needed since playback can
      stay entirely on the frontend side
- [ ] Add a `sound_enabled` (bool, default true) field to `Settings`
      (src-tauri/src/settings.rs) alongside the existing preferences, with a
      checkbox in the Settings panel next to photos-per-session/timer
- [ ] Respect the setting in the frontend: skip playback entirely when
      disabled, rather than muting after the fact

### 10. Packaging & distribution
This is the big unknown the user asked about — details below in its own
section. Concretely:
- [x] Fill in real `tauri.conf.json` bundle metadata: `productName` is now
      "TestStrip" and `identifier` is `com.reedmcilwain.teststrip` (was
      `com.example.lightroomwarmup`). Publisher/description/license fields
      in a `bundle` section are still not filled in.
- [ ] Generate a full icon set from a real source image (`cargo tauri icon`)
- [ ] Decide on code signing (see below) — determines whether recipients see
      scary OS warnings on first launch
- [ ] Produce an actual installer via `cargo tauri build` and test a clean
      install on a machine that isn't the dev machine
- [ ] Decide on update strategy: manual re-install for now, or wire up
      `tauri-plugin-updater` later

### 11. Cross-platform (macOS)
Feasible — details below — but not yet started. Concretely:
- [ ] Confirm `open`/dialog/notify crates behave the same on macOS (they're
      all cross-platform crates, but untested here)
- [ ] `.icns` icon variant
- [ ] macOS-specific first-run instructions for setting a default `.CR2` handler
      (different flow than Windows' "Open with → Always use this app")
- [ ] Code signing + notarization (required for smooth Gatekeeper experience —
      see below, this is the part with real cost/friction)
- [ ] A Mac to actually build and test on (Tauri cross-compilation from
      Windows to macOS isn't practical; needs real macOS hardware or CI)

## Deep dive: is macOS actually possible with this stack?

Yes. Tauri wraps whatever webview the OS already provides (WebView2 on
Windows, WKWebView on macOS, WebKitGTK on Linux) around a Rust backend, and
explicitly supports building for all three from mostly the same codebase.
Nothing in the current Rust code is Windows-specific in a way that would
block macOS — `PathBuf`, `notify` (file watching), `open` (launch default
app / arbitrary executable), and the dialog plugin are all cross-platform
crates that already support macOS and Linux.

What *would* need attention:
- The one line of `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`
  is already conditional and harmless on other platforms.
- App icons need a `.icns` file (Tauri's icon tool generates this alongside
  `.ico` from one source image).
- You need an actual Mac (or macOS CI runner) to build and sign the macOS
  binary — you can't cross-compile a signed, notarized macOS app from Windows.
- The "set as default handler for this file type" flow is different on macOS
  (Get Info → Open With → Change All) and needs its own instructions/UI copy.

Realistic read: cross-platform is a "when you have Mac hardware to test on"
task, not a "rewrite" task.

## Deep dive: what "make this shippable" actually involves

Right now `cargo tauri dev` runs the app from source with debug symbols and
no installer — fine for you, not something you can hand someone else.
`cargo tauri build` is the command that produces real, distributable
artifacts, but a few things need to be true first:

**1. Bundle configuration.** `tauri.conf.json` has a `bundle` section (not
present yet in this project) controlling the app identifier, icons,
publisher name/copyright, and which installer formats to produce:
- Windows: NSIS installer (`.exe`) and/or MSI
- macOS: `.app` bundle, typically wrapped in a `.dmg`
- Linux: `.deb`, `.rpm`, and/or AppImage

**2. Code signing.** This is the part that actually costs money/time and is
worth deciding early:
- *Windows:* an unsigned app triggers a SmartScreen "Windows protected your
  PC" warning on first run. Recipients can click through it, but it looks
  alarming. A proper fix requires an Authenticode code-signing certificate
  (~$100–400+/year from a CA), or accepting the warning for a small-audience
  share (probably fine for "a few friends try this out").
- *macOS:* stricter — an unsigned, unnotarized app is blocked by Gatekeeper
  outright unless the user explicitly right-click → Open's it, or disables
  Gatekeeper protections. To distribute smoothly you need an Apple Developer
  Program membership ($99/year), a Developer ID certificate, and to run the
  build through Apple's notarization service. This is the single biggest
  piece of *process* overhead in going cross-platform and shareable.

**3. A real identifier and icon.** The identifier is now
`com.reedmcilwain.teststrip` (was `com.example.lightroomwarmup`); the
placeholder `.ico` still needs replacing with a real app icon before
anything is shared, even informally.

**4. Distribution channel.** For a small-audience share, the simplest path
is: build the installer, upload it somewhere (a private GitHub Release,
a shared drive link, etc.), and send a direct link — no app store needed.
An app store listing (Microsoft Store / Mac App Store) is a much bigger
lift (review process, sandboxing constraints) and almost certainly not
worth it for this project's audience.

**Practical recommendation for a first shareable build:** stay Windows-only,
skip code signing initially (accept the SmartScreen click-through for a
small trusted audience), get `cargo tauri build` producing a clean NSIS
installer, and share that directly. Revisit signing and macOS once there's
real interest beyond the initial small group.
