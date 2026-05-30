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
just input-doctor
# For short local tests only when persistent NixOS selected-device permissions
# are not configured:
# just unsafe-input-acl
just run-verbose
```

Confirm that release input stops repeat output and that verbose logs include
motion source paths, latency, repeat lifecycle, and provider diagnostics without
logging macro text payloads.
