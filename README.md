# RoZVP

Plants vs. Zombies on Repose + repame. 100 Hz integer-tick sim, live
renamite rigs instead of sprite atlases, pure Repose views. Desktop,
web, and Android from one codebase; the old Bevy path was removed
(history keeps it for reference).

## Run

```bash
cargo run        # desktop, title hub
cargo test       # sim, save, i18n suites
trunk serve      # web
```

Android ships via `cargo-rapk` (`[package.metadata.android]`).

## Structure

```
src/
├── main.rs          # desktop entry
├── lib.rs           # pilot module + wasm/android entries
└── pilot/
    ├── sim.rs       # Sim assembly + 100 Hz tick driver
    ├── combat.rs    # shooters, peas, specials, move/eat, mowers
    ├── economy.rs   # sun, recharge, planting, shovel
    ├── levels.rs    # waves, level flow, awards, advice
    ├── constants.rs # canonical tune values (see docs/PARITY.md)
    ├── views.rs     # board canvas, HUD, menus, overlays
    ├── render.rs    # frame snapshots for the canvas
    ├── rigs.rs      # live zombie rig hosts
    ├── runner.rs    # desktop/web/android entries
    ├── save.rs      # save.ron via game-utils SaveStore
    ├── i18n.rs      # Fluent bundles, translators edit FTL only
    └── audio.rs     # repame-audio engine + synth cue bank
```

## Notes

- **Save**: crash-safe `SaveStore` (`FsStorage`, OPFS on web), same
  `SaveData` shape as before, so old `save.ron` files still load.
- **i18n**: 7 locales under `assets/locales`, per-key English fallback.
- **Audio**: synth cues at boot; real packs drop into the bank later.
- **Stack**: `repose 0.29`, `repame` + `renamite` (path), `game-utils`
  (git rev, `storage`/`save_store` APIs).

## License

GPL-3.0
