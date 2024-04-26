[![CI](https://github.com/evergreen-xch/fast_farmer_config/actions/workflows/ci.yml/badge.svg)](https://github.com/evergreen-xch/fast_farmer_config/actions/workflows/ci.yml)

A Config Generator for FastFarmer Gigahorse
=====

Building
--------

Install Rust by following the instructions at https://www.rust-lang.org/tools/install

Once Rust is installed we can build from source:
```
git clone https://github.com/evergreen-xch/fast_farmer_config.git
cd fast_farmer_config
cargo build --release
sudo cp target/release/ff_config /usr/local/bin/ff_config
```

Running
--------

To generate the farmer config run the below and follow the on-screen prompts:
```
./ff_config
```

> [!TIP]
> To Print Usage / Options use ```./ff_config --help```