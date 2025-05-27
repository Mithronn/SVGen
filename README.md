## Test

```console
cargo test decode_to_svg --release -- --nocapture "assets/BWC.png" "colored"
```

## Build for Web

```console
wasm-pack build --target web --release
```
