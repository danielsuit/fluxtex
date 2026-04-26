Place the Tectonic CLI and optional bundle here so FluXTeX can compile without a host install.

Expected layout:

- `vendor/tectonic/bin/tectonic`
- `vendor/tectonic/bundles/default.zip`
- `vendor/tectonic/bundles/default.bundle`

Runtime lookup order:

1. `FLUXTEX_TECTONIC_BIN`
2. `vendor/tectonic/bin/tectonic`
3. `vendor/tectonic/tectonic`
4. `tectonic` on `PATH`

Bundle lookup order:

1. `FLUXTEX_TECTONIC_BUNDLE`
2. `vendor/tectonic/bundles/default.bundle`
3. `vendor/tectonic/bundles/tectonic-default.bundle`
4. `vendor/tectonic/default.bundle`

The app invokes:

- `tectonic -X compile --keep-logs --keep-intermediates --outdir <temp>/out <temp>/main.tex`

If a local bundle is present, the app adds:

- `--bundle vendor/tectonic/bundles/default.bundle`
