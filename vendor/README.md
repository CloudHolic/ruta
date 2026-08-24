# vendor/

Third-party assets. **Never modify anything under this directory, for any reason.**
If the test suite needs patching, put an overlay or prelude under `conformance/` instead.

## `puc-lua/`

PUC-Lua 5.5.1 C sources. Extracted from `lua-5.5.1.tar.gz`.

- Source: https://www.lua.org/ftp/lua-5.5.1.tar.gz
- Version: 5.5.1, released 2026-07-24
- SHA256 (tarball): `1c4b4068d67061f2a2231ad2b5422e77acea1487ea9890f6320af614f4373dce`
  Verified against the checksum published on lua.org/ftp.
- License: MIT. Copyright (C) 1994-2026 Lua.org, PUC-Rio.

## `lua-tests/`

The official test suite for Lua 5.5.1. Extracted from `lua-5.5.1-tests.tar.gz`.

- Source: https://www.lua.org/tests/lua-5.5.1-tests.tar.gz
- Version: 5.5.1
- SHA256 (tarball): `da07b543872dc0bb2ff12aabd0c248578d78df3eb6b67efdc537a46d455c7f31`
  Computed locally — lua.org publishes no checksums for the test suites. This records the
  bytes we vendored so that later drift is detectable; it is not a verification against
  upstream.
- License: MIT, same as the Lua distribution.
