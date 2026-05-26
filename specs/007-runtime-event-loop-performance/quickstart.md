# Quickstart: Runtime Event Loop Performance

Run automated verification:

```sh
nix develop -c cargo fmt --check
nix develop -c cargo clippy --all-targets -- -D warnings
nix develop -c cargo test
nix flake check
```

Run live diagnostics:

```sh
just unsafe-input-acl
just run-verbose
```

Confirm that release input stops repeat output and that verbose logs include
motion source paths, latency, repeat lifecycle, and provider diagnostics without
logging macro text payloads.
