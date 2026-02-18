---
name: review
description: Review local changes or a pull request
---

If no arguments are passed, review the local changes by looking at the diff between the base branch - master by default - and the current branch.
If arguments are passed, review pull request #$ARGUMENTS by fetching it and seeing its details with `gh pr view` and `gh pr diff`.

When reviewing, analyze for:

1. **Relay Client Correctness**
   - `SimpleRuntimeVersion` constants match actual on-chain runtime versions
   - `codegen_runtime.rs` files are regenerated (not manually edited)
   - Transaction encoding matches target chain expectations
   - BlakeTwo256 workaround applied after codegen (`::sp_runtime::generic::Header<::core::primitive::u32, ::sp_runtime::traits::BlakeTwo256>`)

2. **Bridge Configuration**
   - Message lane IDs are correct (`00000001` for Kusama-Polkadot, `00000002` for Rococo-Westend)
   - Bridge module structure follows existing patterns in `substrate-relay/src/bridges/`
   - Header relay, message relay, and parachain relay components are consistent per direction
   - Both directions of a bridge are updated symmetrically

3. **Code Quality**
   - Rust idioms and async patterns (async-std, futures)
   - Error handling with `anyhow::Result`; no `unwrap()`/`expect()` in relay logic
   - Logging uses `log::` macros with appropriate `target: LOG_TARGET`
   - No unnecessary allocations in hot relay loops

4. **Security**
   - Signer keys not logged or exposed in error messages
   - Transaction construction uses proper nonce management
   - Balance checks before submitting transactions
   - No secrets in committed code or configuration

5. **CI/CD Impact**
   - Changes to `.github/workflows/` are intentional and correct
   - Dockerfile changes don't break the build pipeline
   - `deny.toml` changes don't introduce disallowed dependencies

6. **Release Impact**
   - If `substrate-relay/Cargo.toml` version is bumped, check for `A-release` label
   - Verify bundled chain versions are documented in PR description
   - Check that `Cargo.lock` is updated consistently
   - Verify `scripts/regenerate_runtimes.sh` wasn't accidentally modified

Provide specific feedback with file paths and line numbers.
