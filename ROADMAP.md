# Lightroom Warmup — Roadmap to Shareable MVP

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
- [ ] Remember last-used archive/export folders across restarts (quality of
      life; currently reselected every launch)

### 3. Challenge content
- [ ] Revisit the starter challenge list — the first draft is a placeholder;
      worth a pass once a few real sessions have been run
- [ ] Decide if some sessions should force at least one challenge from a
      given "family" (e.g. always at least one b&w challenge per week) —
      explicitly deferred earlier when "flat list, no categories" was chosen;
      revisit if variety turns out to be a problem in practice

### 4. Visual design
Right now the UI is unstyled functional HTML (system font, default blue
buttons, no real visual identity). To actually be "designed":
- [ ] Decide on a visual direction (this is a small always-on-top utility
      window — should it look like a tool, a game, something else?)
- [ ] Real typography/spacing/color pass, likely dark-mode-aware
- [ ] Consider whether the fixed-size window model still holds once Settings
      has grown (it's already resizable now — decide if that's final)
- [ ] Icon: currently only a placeholder `.ico`; needs a real app icon in
      all required formats per platform (see Packaging below)

### 5. Packaging & distribution
This is the big unknown the user asked about — details below in its own
section. Concretely:
- [ ] Fill in real `tauri.conf.json` bundle metadata (publisher, description,
      license, proper `identifier` — currently `com.example.lightroomwarmup`
      which should not ship as-is)
- [ ] Generate a full icon set from a real source image (`cargo tauri icon`)
- [ ] Decide on code signing (see below) — determines whether recipients see
      scary OS warnings on first launch
- [ ] Produce an actual installer via `cargo tauri build` and test a clean
      install on a machine that isn't the dev machine
- [ ] Decide on update strategy: manual re-install for now, or wire up
      `tauri-plugin-updater` later

### 6. Cross-platform (macOS)
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

**3. A real identifier and icon.** `com.example.lightroomwarmup` and the
placeholder `.ico` are dev-only stand-ins; both should be replaced before
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
