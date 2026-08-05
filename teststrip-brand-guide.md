# TestStrip — Brand Guide v1

## Core concept

TestStrip is a darkroom, translated into software. The app's job is to warm up an eye before real editing work starts, the same way a physical test strip warmed up a print before committing paper and chemistry to it.

The interaction model should borrow directly from the physical process: an image isn't just "loaded," it's developed. It enters desaturated or tinted, sits in a bath, then resolves into full color and clarity as the session begins. That single motion — submersion to reveal — is the app's signature gesture and should recur anywhere a new image or challenge appears.

Dark-first. Warm accents, not neon ones. Nothing about this should feel clinical.

---

## Color palette

Darkroom base, with analog warmth layered in rather than a cold monochrome.

| Role | Color | Hex | Notes |
|---|---|---|---|
| Base / background | Near-black | `#121010` | Primary surface, feels like a room with the lights off |
| Secondary surface | Deep charcoal | `#1D1A18` | Cards, panels, raised surfaces |
| Primary accent | Safelight red | `#B23A2E` | Active states, the "developing" glow, primary CTA |
| Secondary accent | Amber | `#C98A4B` | Warnings, secondary actions, warm highlight |
| Text / light surface | Film cream | `#EDE4D6` | Primary text on dark, never pure white |
| Muted text | Warm gray | `#8C837A` | Secondary text, metadata, timestamps |

Avoid pure white and pure black entirely — both read as digital rather than photographic. Cream and near-black keep the analog warmth even in high-contrast moments.

Safelight red should be used sparingly and mean something (an active process, a live session) rather than being a generic brand color slapped everywhere. Treat it like the actual safelight in a darkroom: it's there because you need to see, not for decoration.

---

## Typography

Carrying the same serif/monospace contrast already established in the Test Strip series, extended into the UI itself.

- **Display / editorial — Newsreader.** Already doing work in the Instagram series masthead. Use for the app name, section headers, and anywhere the tool needs a human, editorial voice.
- **Functional / technical — a monospace.** Metadata, timestamps, the Target Folder field, challenge text, slider values. Suggest **IBM Plex Mono** for something clean with a little personality, or **Space Mono** if you want slightly more quirk. Both are free and pair well against Newsreader without competing with it.

The contrast itself is the point: Newsreader carries the feeling, the monospace carries the facts. Every screen should have both voices present, not just one.

---

## Motifs and interaction language

These are the recurring visual ideas that should show up across the UI, not just the marketing:

- **The bath.** The core reveal animation. A new image or challenge enters submerged — desaturated, or under a faint red wash — then develops into full clarity over a second or two. This should be the transition whenever something new surfaces, not just a launch animation.
- **Exposure gradient as progress.** Instead of a generic loading bar, use a tonal step gradient (light to dark, like an actual test strip) to represent progress, loading, or session duration.
- **Trays, not cards.** Where most apps use flat rounded rectangles, lean into shallow tray/dish shapes for containers, slightly soft-edged, evoking a chemical bath rather than a UI card.
- **Grain.** A very low-opacity grain texture over dark surfaces keeps things from feeling like flat digital color. Subtle, not a filter effect.
- **Contact sheet grid.** Any view showing multiple images (history, past sessions, the target folder) should reference a contact sheet layout — a tight, even grid — rather than a generic gallery view.

### Waiting and progress

Darkroom waiting has its own physical language, distinct from a generic spinner or progress bar. Three ideas worth building into the UI specifically:

- **Agitation.** During development, a print is rocked gently and rhythmically in the tray to keep chemistry even across the surface. Any loading state should move like this — a slow, subtle rock or wobble rather than a spin. It should feel like something is being tended to, not just buffering.
- **Darkroom timer.** These are mechanical dial timers, not digital countdowns — a needle sweeping around a circular face. Any countdown or session-length indicator should take this dial form rather than a linear bar. It's also a natural fit for showing challenge time remaining during an active session.
- **Safelight pulse.** Real safelights have a faint, uneven pulse rather than a steady glow. This should be the idle/standby state — the app "breathing" quietly when it's waiting for input, rather than sitting static or fully dark.

Together these three cover the app's main waiting states: agitation for active processing, the dial timer for anything counting down, and the safelight pulse for idle. None of them should read as a generic loading indicator borrowed from elsewhere.

---

## Logo direction (suggestions, not designs)

A few directions worth sketching against, since you're taking the actual design work on yourself:

**Wordmark only**
- Literal split treatment: "Test" in a normal weight, "Strip" rendered as a tonal gradient (light to dark steps), so the word itself becomes a test strip. Carries the series masthead idea directly into the product name.
- Alternative: the whole wordmark in Newsreader, but with a thin gradient bar sitting underneath like a strip of film beneath the type, rather than gradient in the letters themselves. Lower risk of readability issues at small sizes.

**Standalone icon**
- A vertical strip with 4–5 graduated tone steps, abstracted down to a simple rectangle with a gradient. Reads at any size, and is literally the object the brand is named after.
- A single frame with a wavy line across the lower third, suggesting a print partially submerged in a tray. More illustrative, less scalable at small sizes (favicon territory).
- A clothespin/clip silhouette, referencing prints hung to dry. Good secondary mark, maybe better suited to a bookmark/pin icon inside the app than the primary logo.

**Combination mark**
- Icon (the graduated strip) sitting to the left of the wordmark, sized as a small modular unit so it can be cropped to a standalone app icon without redesign. This is probably the most practical option since you'll need something that works as an app icon, a favicon, and a full lockup.

---

## Open questions for next pass

- Whether the amber accent shows up as often as red, or stays rare (used only for warnings/secondary states)
- Exact easing/duration for the "bath" reveal animation once you're building it in the UI
- Whether the Target Folder field gets its own visual treatment (given it's a meaningful data point) or stays styled like any other metadata
