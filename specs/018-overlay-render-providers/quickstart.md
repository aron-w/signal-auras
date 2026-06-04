# Quickstart: Overlay Render Providers

## Automated Verification

Run targeted tests during implementation:

```sh
cargo fmt --check
cargo test -p signal-auras-core overlay
cargo test -p signal-auras-lua overlay
cargo test --test lua_api overlay
cargo test --test rust_library overlay
cargo test
cargo clippy --all-targets -- -D warnings
```

Run Nix verification where feasible:

```sh
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
nix flake check
```

## Nested KDE Overlay Smoke Test

Run the fast compositor smoke check in a nested virtual KWin session:

```sh
just overlay-smoke-nested
```

This starts a private D-Bus session and virtual KWin output, renders the native
overlay smoke bar, verifies that KWin can place the overlay window, asks the QML
scene to save a rendered image, converts that image to raw RGB with ImageMagick,
and checks the expected magenta pixel region. It does not open the screen-share
portal and is intended for renderer/placement regression testing during
implementation.

## Live KDE Portal Overlay Smoke Test

Run the compositor smoke check from a KDE Plasma Wayland session:

```sh
nix develop -c cargo run -p signal-auras-cli -- doctor overlay
```

When the screen-share portal opens, select the entire screen. The smoke test
renders a known magenta progress bar through the native overlay provider,
captures a screen frame through the screen-read provider, checks the expected
pixel region, and then cleans up the overlay. Selecting a single game/window may
exclude overlay surfaces from the captured frame and should fail the pixel
check.

If `doctor overlay` reports `matched_pixels=0` but `just overlay-smoke-nested`
passes, the native renderer can spawn, place, and paint under KDE Wayland, and
the remaining failure is in the real portal capture path or selection. If both
fail, debug native QML window creation, KWin placement, and overlay colors
before testing PoE2.

## Lua Example Shape

The PoE2 controller example keeps existing tracker declarations and may add an overlay declaration like:

```lua
sa.overlay.mount({
  id = "poe2_status",
  scope = poe2_scope,
  provider = "native",
  surface = "overlay",
  visuals = {
    {
      id = "heavy_stun",
      kind = "progress_bar",
      bind = { tracker = "heavy_stun", field = "progress_percent" },
      rect = { x = 1640, y = 1590, w = 600, h = 22 },
      opacity = 0.72,
      fill = "#d8b84c",
      background = "#101820",
      label = { visible = true },
      inactive = { opacity = 0.25 },
    },
    {
      id = "refutation",
      kind = "progress_bar",
      bind = { tracker = "refutation_cooldown", field = "remaining_ms" },
      rect = { x = 1640, y = 1620, w = 600, h = 22 },
      opacity = 0.72,
      fill = "#5aa7ff",
      background = "#101820",
      label = { visible = true },
      ready = { fill = "#4ade80", opacity = 0.85 },
      inactive = { opacity = 0.25 },
    },
  },
})
```

When editing `examples/poe2.lua`, preserve existing user edits and append or merge overlay declarations surgically.

## Manual KDE/PoE2 Verification

1. Start a KDE Plasma Wayland session on NixOS.
2. Start PoE2 in the configured fullscreen layout.
3. Run the controller example with verbose diagnostics.
4. Confirm two translucent bars appear above the game only while PoE2 focus is trusted.
5. Confirm Heavy Stun updates from the existing `heavy_stun.progress_percent` tracker.
6. Confirm Refutation uses cooldown style while cooling down and ready style when ready.
7. Move and click through the overlay area and confirm mouse input reaches the game.
8. Switch focus away from PoE2 and confirm overlays hide or become inactive with diagnostics.
9. Stop Signal Auras and confirm overlay surfaces are cleaned up.

## Expected Closed States

- Unknown provider: script validation error.
- Future provider without adapter: runtime unavailable diagnostic and no active surface.
- Denied permission: denied diagnostic and no active surface.
- Inactive or stale focus: inactive diagnostic and no active surface.
- Missing or stale tracker source: stale or missing-source diagnostic and no active visual.
