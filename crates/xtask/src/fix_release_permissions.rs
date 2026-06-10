use anyhow::{bail, Context};

const PATH: &str = ".github/workflows/release.yml";

/// Patch release.yml after `dist generate` to fix Scorecard findings.
///
/// 1. Permissions: dist generates `permissions: contents: write` at the top level; this patches
///    it to `contents: read` and grants `contents: write` only to plan and host jobs.
/// 2. Pinned-Dependencies: replaces `curl | sh` patterns with download-verify-execute so
///    Scorecard recognises the scripts as hash-pinned.
pub fn run() -> anyhow::Result<()> {
    let content =
        std::fs::read_to_string(PATH).with_context(|| format!("failed to read {PATH}"))?;

    let patched = apply_patches(&content)?;

    std::fs::write(PATH, patched).with_context(|| format!("failed to write {PATH}"))?;

    println!("Patched {PATH}: scoped permissions and pinned curl downloads");
    Ok(())
}

fn apply_patches(content: &str) -> anyhow::Result<String> {
    // Each tuple is (from, to). If `from` is absent but `to` is already present,
    // the patch is a no-op (already applied). If neither is present, the file has drifted.
    let patches = [
        (
            "name: Release\npermissions:\n  \"contents\": \"write\"",
            "name: Release\npermissions:\n  \"contents\": \"read\"",
        ),
        (
            "  plan:\n    runs-on:",
            "  plan:\n    permissions:\n      \"contents\": \"write\"\n    runs-on:",
        ),
        (
            "  host:\n    needs:",
            "  host:\n    permissions:\n      \"contents\": \"write\"\n    needs:",
        ),
        // Scorecard Pinned-Dependencies: replace curl|sh with download-verify-execute
        (
            concat!(
                "      - name: Install dist\n",
                "        # we specify bash to get pipefail; it guards against the `curl` command\n",
                "        # failing. otherwise `sh` won't catch that `curl` returned non-0\n",
                "        shell: bash\n",
                "        run: \"curl --proto '=https' --tlsv1.2 -LsSf",
                " https://github.com/axodotdev/cargo-dist/releases/download/v0.32.0/cargo-dist-installer.sh | sh\"",
            ),
            concat!(
                "      - name: Install dist\n",
                "        shell: bash\n",
                "        run: |\n",
                "          curl --proto '=https' --tlsv1.2 -LsSf",
                " https://github.com/axodotdev/cargo-dist/releases/download/v0.32.0/cargo-dist-installer.sh",
                " -o /tmp/cargo-dist-installer.sh\n",
                "          echo \"b657cf8c04a8b7bc28f39d220f7e6dd11bbd2bdb072c552262bd9ccf597261b5",
                "  /tmp/cargo-dist-installer.sh\" | sha256sum -c\n",
                "          sh /tmp/cargo-dist-installer.sh",
            ),
        ),
        (
            "            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y",
            concat!(
                "            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup-init.sh\n",
                "            echo \"6c30b75a75b28a96fd913a037c8581b580080b6ee9b8169a3c0feb1af7fe8caf",
                "  /tmp/rustup-init.sh\" | sha256sum -c\n",
                "            sh /tmp/rustup-init.sh -y",
            ),
        ),
        // Scorecard Pinned-Dependencies: dist emits actions/attest@v4 unpinned; pin to the
        // commit SHA that v4 resolves to (all other actions are already SHA-pinned).
        (
            "uses: actions/attest@v4",
            "uses: actions/attest@59d89421af93a897026c735860bf21b6eb4f7b26",
        ),
    ];

    let mut result = content.to_owned();
    for (from, to) in patches {
        if result.contains(from) {
            result = result.replace(from, to);
        } else if !result.contains(to) {
            bail!("{PATH} does not contain expected string — has the file drifted?\n  Expected: {from:?}");
        }
    }
    Ok(result)
}
