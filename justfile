set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

# Show the project guide and command menu.
default:
    @just guide

# Render a shell-friendly guide for what this project can do right now.
guide:
    @printf '%s\n' '# Signal Auras command guide'
    @printf '%s\n' '# v1 is a terminal-started Lua hotkey runner with a mock-friendly Wayland adapter.'
    @printf '%s\n' '# Real compositor global shortcuts, active-process metadata, and synthesized input are future adapter work.'
    @printf '\n%s\n' '# Daily checks'
    @printf '%s\n' 'just check        # fmt + clippy + tests'
    @printf '%s\n' 'just test         # run all automated tests'
    @printf '%s\n' 'just fmt          # check Rust formatting'
    @printf '%s\n' 'just lint         # run clippy with warnings denied'
    @printf '\n%s\n' '# Run the CLI'
    @printf '%s\n' 'just run          # run the scoped poe2 example and wait for Ctrl-C'
    @printf '%s\n' 'just run-verbose  # run the scoped poe2 example with debug event logs'
    @printf '%s\n' 'just unsafe-input-acl # temporarily grant this user evdev/uinput access'
    @printf '%s\n' 'just run-prompt   # run the scope-free example and exercise terminal consent'
    @printf '%s\n' 'just sigint-smoke # send SIGINT to the runner and verify final stats print'
    @printf '\n%s\n' '# Failure and verification flows'
    @printf '%s\n' 'just failures     # run expected failure scenarios from quickstart'
    @printf '%s\n' 'just story-tests  # run contract/integration tests for the runner stories'
    @printf '%s\n' 'just manual       # print manual Wayland verification steps'
    @printf '\n%s\n' '# Spec Kit context'
    @printf '%s\n' 'just tasks        # show remaining task checkboxes'
    @printf '%s\n' 'just spec         # print the feature spec'

# Enter the reproducible development shell.
dev:
    nix develop

# Run Rust formatting check only.
fmt:
    nix develop -c cargo fmt --check

# Format Rust source in-place.
fmt-fix:
    nix develop -c cargo fmt

# Run clippy on every target with warnings treated as errors.
lint:
    nix develop -c cargo clippy --all-targets -- -D warnings

# Run every automated test target.
test:
    nix develop -c cargo test

# Run the normal verification gate used before handing off work.
check: fmt lint test
    @printf '%s\n' '# check complete: fmt, clippy, and tests passed'

# Run the contract and integration tests that exercise the Lua runner stories.
story-tests:
    nix develop -c cargo test --test lua_api --test cli_runner --test runner_flow --test rust_library

# Run the scoped sample. Press Ctrl-C to stop and print final stats.
run file="examples/poe2-hideout.lua":
    @printf '%s\n' '# running scoped Lua example; press Ctrl-C to stop'
    nix develop -c cargo run -p signal-auras-cli -- run {{file}}

# Run the scoped sample with verbose event logs for provider/input debugging.
run-verbose file="examples/poe2-hideout.lua":
    @printf '%s\n' '# running scoped Lua example with verbose debug logs; press Ctrl-C to stop'
    nix develop -c cargo run -p signal-auras-cli -- run --verbose {{file}}

# Temporarily grant the current user access to unsafe input devices for local testing.
# These ACLs are reset by reboot, device replug, or udev permission changes.
unsafe-input-acl:
    @printf '%s\n' '# granting temporary ACLs for /dev/input/event* and /dev/uinput'
    sudo modprobe uinput || true
    sudo setfacl -m "u:$USER:r" /dev/input/event*
    sudo setfacl -m "u:$USER:rw" /dev/uinput
    @printf '%s\n' '# done; run the app as your normal user, not with sudo'

# Run the scope-free sample so the terminal consent prompt is shown.
run-prompt:
    @printf '%s\n' '# choose process scope, explicit GLOBAL, or cancel in the terminal prompt'
    nix develop -c cargo run -p signal-auras-cli -- run examples/prompt-scope.lua

# Send SIGINT to the scoped runner and confirm it prints final Ctrl-C stats.
sigint-smoke seconds="2":
    @printf '%s\n' '# timeout sends SIGINT; exit 124 from timeout is expected after the runner prints final_summary'
    -nix develop -c timeout -s INT {{seconds}}s cargo run -p signal-auras-cli -- run examples/poe2-hideout.lua

# Run quickstart failure scenarios that should exit before registration.
failures:
    @printf '%s\n' '# zero args: expect argument_validation'
    -nix develop -c cargo run -p signal-auras-cli --
    @printf '\n%s\n' '# two paths: expect argument_validation'
    -nix develop -c cargo run -p signal-auras-cli -- run examples/poe2-hideout.lua examples/prompt-scope.lua
    @printf '\n%s\n' '# invalid Lua shape: expect script_validation'
    @printf '%s' 'not lua' > /tmp/signal-auras-invalid.lua
    -nix develop -c cargo run -p signal-auras-cli -- run /tmp/signal-auras-invalid.lua
    @printf '\n%s\n' '# no hotkeys: expect script_validation'
    @printf '%s' 'return { hotkeys = {} }' > /tmp/signal-auras-no-hotkeys.lua
    -nix develop -c cargo run -p signal-auras-cli -- run /tmp/signal-auras-no-hotkeys.lua
    @printf '\n%s\n' '# ambient Lua API: expect sandbox denial'
    @printf '%s' 'return { hotkeys = {}, leak = os.getenv("HOME") }' > /tmp/signal-auras-ambient.lua
    -nix develop -c cargo run -p signal-auras-cli -- run /tmp/signal-auras-ambient.lua
    @printf '\n%s\n' '# non-interactive missing scope: expect scope_prompt error'
    -nix develop -c cargo run -p signal-auras-cli -- run examples/prompt-scope.lua

# Print the manual Wayland verification procedure.
manual:
    sed -n '1,260p' tests/compositor/manual-wayland-verification.md

# Show the current Spec Kit task checklist.
tasks:
    rg -n '^- \[[ Xx]\] T' specs/001-lua-hotkey-runner/tasks.md

# Print the feature spec.
spec:
    sed -n '1,260p' specs/001-lua-hotkey-runner/spec.md

# Print the implementation plan.
plan:
    sed -n '1,260p' specs/001-lua-hotkey-runner/plan.md

# Show changed files in the current worktree.
status:
    git status --short
