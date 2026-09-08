# RELEASE (template)

Release/publish workflow for a pattern-A native module. Copy the yaml
below into `.github/workflows/release.yml` of YOUR module repository,
set the secrets, and drop the `dry run` guard when ready.

Versioning contract: the main package and every platform package share
ONE version; `optionalDependencies` pins the platform packages EXACTLY
(`"0.1.0"`, not `"^0.1.0"`). Bump everything together.

## Order of operations (per release)

1. Bump versions: main `package.json` + all platform pins.
2. Tag `v<version>` and push the tag.
3. CI matrix builds the cdylib on each OS, runs `pack-platforms`,
   and publishes: **platform packages first, then the main package**
   (a main release with missing platform pins is the classic breakage).
4. Attach the packed tarballs (`.tgz`) + SHA256 to the GitHub Release.

## Workflow template

```yaml
name: release

on:
  push:
    tags: ["v*"]

jobs:
  build:
    strategy:
      matrix:
        include:
          - { os: windows-latest, triple: win32-x64-msvc, src: target/release/bffi_example_native.dll }
          - { os: ubuntu-latest, triple: linux-x64-gnu, src: target/release/libbffi_example_native.so }
          - { os: macos-latest, triple: darwin-aarch64, src: target/release/libbffi_example_native.dylib }
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: oven-sh/setup-bun@v2
        with: { bun-version: 1.4.0 }
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release
      - run: bun scripts/pack-platforms.ts --src ${{ matrix.src }} --triple ${{ matrix.triple }}
      - run: cd platform/@scope/native-template-${{ matrix.triple }} && npm publish --access public
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
      - uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.triple }}
          path: platform/@scope/native-template-${{ matrix.triple }}/*.tgz

  publish-main:
    needs: build
    runs-on: ubuntu-latest
    environment: npm   # optional: require manual approval here
    steps:
      - uses: actions/checkout@v4
      - uses: oven-sh/setup-bun@v2
        with: { bun-version: 1.4.0 }
      - uses: actions/download-artifact@v4
      - run: npm publish --access public
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
```

Replace `@scope` / `native-template` / the `src` binary names with your
crate's values. Until then keep `private: true` in the main package
and run `npm pack --dry-run` only.
