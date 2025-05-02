Simple cross-platform image viewer, using winit for supported platforms, and custom implementation for HelenOS.

For HelenOS, you will need to build the HelenOS toolchain and then set the environment variable HELENOS_INCLUDE_BASE to a folder with HelenOS C library headers (the `export-dev/include` folder in your HelenOS build).

Then build with `HELENOS_INCLUDE_BASE=$HOME/dev/helenos/amd64/export-dev/include cargo +mycustomtoolchain build --target x86_64-unknown-helenos --release`