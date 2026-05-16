use anyhow::{bail, Context};

const PATH: &str = ".github/workflows/release.yml";

/// Patch release.yml after `dist generate` to restore scoped GitHub workflow permissions.
///
/// dist generates `permissions: contents: write` at the top level; Scorecard flags this.
/// This patches it to `contents: read` and grants `contents: write` only to the two jobs
/// that actually create GitHub Releases (plan and host).
pub fn run() -> anyhow::Result<()> {
    let content = std::fs::read_to_string(PATH)
        .with_context(|| format!("failed to read {PATH}"))?;

    let patched = apply_patches(&content)?;

    std::fs::write(PATH, patched)
        .with_context(|| format!("failed to write {PATH}"))?;

    println!("Patched {PATH}: scoped contents:write to plan and host jobs only");
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
