# TestStrip (Tauri POC)

A minimal Windows desktop companion for Lightroom warmup sessions. It randomly picks three `.CR2` files from an archive folder, opens each in Lightroom via the OS default handler, watches an export folder for the expected JPG, and walks you through the session with a small always-on-top UI.

## Requirements

- Rust toolchain
- Tauri prerequisites for Windows (WebView2 + MSVC build tools)
- `cargo install tauri-cli`
- Lightroom set as the default handler for `.CR2` files

## Run (dev)

```bash
cd src-tauri
cargo tauri dev
```

The frontend is served from `dist/` and requires no additional build step.

## Usage

1. Launch the app, choose your **archive folder** (root of your RAW files) and **export folder** (where session subfolders get created), then click **Start**.
2. Lightroom opens the first RAW.
3. Export a JPG to the session folder. The app expects the filename to match the RAW basename (e.g., `IMG_1234.jpg`).
4. Repeat for all three photos. A summary view lists any exported JPGs.

## Lightroom default handler

Ensure `.CR2` files open in Lightroom by default:

1. Right-click a `.CR2` file in Explorer.
2. Choose **Open with → Choose another app**.
3. Select Lightroom and enable **Always use this app**.

## Notes

- The window is always-on-top by default.
- If the timer expires, you can continue working or advance manually.
- Exports are detected by watching the session export folder for the expected JPG name.
