# hydraDX-math-helpers

![example workflow](https://github.com/galacticcouncil/HydraDX-math/actions/workflows/tests.yml/badge.svg)
![GitHub tag (latest by date)](https://img.shields.io/github/v/tag/galacticcouncil/HydraDX-math)

A collection of utilities to make performing liquidity pool calculations more convenient.

### Development

This crate uses the [`rug` crate](https://crates.io/crates) for arbitrary precision math in tests
which depends on GMP under the hood. You will thus need to install the dependencies in order to be
able to run the tests.

For Debian based systems
```sh
sudo apt install diffutils gcc m4 make
```
should do the trick.

See the `rug` docs [here](https://crates.io/crates/rug#using-rug)
