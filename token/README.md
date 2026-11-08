# CLIFF demo token

A NEP-141 token for testnet demos. It has no value and 0 decimals, and the whole supply goes to `owner_id` at `new`. It is deployed at `cliffhanger-token-8nov.testnet`.

```
RUSTUP_TOOLCHAIN=1.86.0 cargo near build non-reproducible-wasm --no-abi
```
