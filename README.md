# qu-core

Shared foundation of [AniQu](https://github.com/simonroqu/AniQu), BoxQu and [Tubequ](https://github.com/simonroqu/tubequ):
GTK4 theme (follows [Omarchy](https://omarchy.org) live), shared CSS, icon set, open/miniplayer zoom animation,
navigation bubble, search popup, smooth scrolling and GPU upscaling for the embedded mpv player.

Not meant to be used on its own; the apps pull it in as a git dependency:

```toml
qu-core = { git = "https://github.com/simonroqu/quCore", tag = "v0.1.0" }
```

## Credits

- [FSRCNNX](https://github.com/igv/FSRCNN-TensorFlow) upscaling shader by igv (GPL-3.0/LGPL-3.0, `data/shaders/`)
- [Anime4K](https://github.com/bloc97/Anime4K) upscaling shaders by bloc97 (MIT, `data/shaders/`)

## License

[GPL-3.0-or-later](LICENSE)
