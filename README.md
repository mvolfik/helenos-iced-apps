GUI Rust applications for HelenOS using the Iced framework, with custom mapping to HelenOS runtime (or winit for testing on other platforms).

For HelenOS, you will need to build the HelenOS toolchain and then set the environment variable HELENOS_INCLUDE_BASE to a folder with HelenOS C library headers (the `export-dev/include` folder in your HelenOS build).

Then build with `HELENOS_INCLUDE_BASE=$HOME/dev/helenos/amd64/export-dev/include cargo +mycustomtoolchain build --target x86_64-unknown-helenos --release`

Current list of applications:

- `life`: Conway's game of life, from https://github.com/iced-rs/iced/blob/b1c13e285ee6009a3c547ffb12038b0ca91c4d35/examples/game_of_life/src/main.rs
- `imageviewer-rs`: a simple image viewer with file browser included

---

A part of this repository are Noto fonts downloaded from https://fonts.google.com/noto . These files are licensed under the SIL Open Font License, Version 1.1, see fonts/LICENSE for more details.
