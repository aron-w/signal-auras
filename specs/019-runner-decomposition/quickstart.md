# Quickstart: Runner Architecture Decomposition

1. Confirm lifecycle cleanup, callback responsiveness, and focus policy tests pass before structural movement:

   ```sh
   cargo test
   ```

2. Implement one boundary extraction at a time and run targeted tests for that boundary.

3. Run final verification:

   ```sh
   cargo fmt --check
   cargo test
   cargo clippy --all-targets -- -D warnings
   ```

4. When Nix evaluation is available, run:

   ```sh
   nix develop -c cargo fmt --check
   nix develop -c cargo test
   nix develop -c cargo clippy --all-targets -- -D warnings
   nix flake check
   ```
