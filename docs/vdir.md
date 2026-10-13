# Virtual Directory Reference

Weaver uses a uniform string syntax—called a **Virtual Directory Reference**—to reference directories and files across local filesystems, compressed archives, HTTP(S) endpoints, and Git repositories.

Wherever Weaver accepts a virtual directory reference string (CLI flags, `.weaver.toml` configuration, or registry manifests), it resolves and materializes the referenced location into a local path on disk so that schema resolution, Rego policy evaluation, and Jinja template rendering can operate uniformly regardless of where the files are hosted.

Weaver refers to these references as the `vdir` type in some documentation.

## String Format

A virtual directory string follows the general pattern:

```text
<source>['@' <refspec>]['[' <sub_folder> ']']
```

| Component | Required | Description |
| :--- | :--- | :--- |
| `<source>` | Yes | Base location: a local filesystem path or an `http://` / `https://` URL. |
| `@<refspec>` | No | Git branch name, tag name, or full 40-character hexadecimal commit SHA. Only used when `<source>` resolves to a Git repository (ignored for archives and local paths). |
| `[<sub_folder>]` | No | Relative path to a directory inside a Git repository or archive to use as the root of the virtual directory. Ignored for plain local folders/files and single remote files. |

### Quick Examples

| Source Type | Example `vdir` String |
| :--- | :--- |
| **Local directory** | `./model` or `/path/to/registry` |
| **Local file** | `./manifest.yaml` or `./policies/semconv.rego` |
| **Local archive** | `./semconv-release.tar.gz[model]` or `./archive.zip` |
| **Remote archive** | `https://github.com/open-telemetry/semantic-conventions/archive/refs/tags/v1.27.0.zip[model]` |
| **Remote file** | `https://example.com/schemas/my-registry/1.0.0/manifest.yaml` |
| **GitHub release asset** | `https://github.com/my-org/my-semconv/releases/download/v1.0.0/registry.tar.gz` |
| **Git repository (default branch)** | `https://github.com/open-telemetry/semantic-conventions.git` |
| **Git repository (branch/tag + sub-folder)** | `https://github.com/open-telemetry/semantic-conventions.git@v1.27.0[model]` |
| **Git repository (commit SHA + sub-folder)** | `https://github.com/open-telemetry/semantic-conventions@0123456789abcdef0123456789abcdef01234567[model]` |

---

## Priority Order for Source Interpretation

When parsing a virtual directory reference string, Weaver first extracts `<source>`, optional `@<refspec>`, and optional `[<sub_folder>]`,
then attempts to classify the `<source>` using the following **strict priority order**:

### 1. HTTP(S) URLs (`<source>` starts with `http://` or `https://`)

1. **Remote Archive** — `<source>` ends with `.zip` or `.tar.gz`.
   - Uses `<source>` as the archive download URL and `[<sub_folder>]` (if present) as the inner directory to extract.
   - *Note: `@<refspec>` is ignored for remote archives.*
2. **Git Repository** — `<source>` does **not** end with `.zip` or `.tar.gz`, **and** at least one of the following is true:
   - `<source>` ends with `.git`
   - `@<refspec>` is present
   - `[<sub_folder>]` is present
3. **Remote File (`RemoteFile`)** — Any other `http://` or `https://` URL (does not end with `.zip`, `.tar.gz`, or `.git`, and has neither `@<refspec>` nor `[<sub_folder>]`).
  *Note: Weaver MAY attempt to treat this as a directory and look for named files via URL manipulation, e.g. looking for http://example.com/myschema/resolved.yaml from references given in http://example.com/myschema/manifest.yaml.*

> [!IMPORTANT]
> Because the presence of `@<refspec>` or `[<sub_folder>]` classifies a non-archive HTTP(S) URL as a **Git repository**, you do not need a `.git` suffix when specifying a refspec or sub-folder (for example, `https://github.com/open-telemetry/semantic-conventions@v1.27.0[model]`).
>
> Conversely, a bare Git repository URL without `@<refspec>` or `[<sub_folder>]` **must** end with `.git` (for example, `https://github.com/open-telemetry/semantic-conventions.git`), otherwise Weaver will interpret it as a single **Remote File**.

### 2. Local Paths (`<source>` does not start with `http://` or `https://`)

1. **Local Archive (`LocalArchive`)** — `<source>` ends with `.zip` or `.tar.gz`.
   - Uses `<source>` as the local archive path and `[<sub_folder>]` (if present) as the inner directory to extract.
   - *Note: `@<refspec>` is ignored for local archives.*
2. **Local Folder or File (`LocalFolder`)** — Any other local path.
   - Uses `<source>` directly on the local filesystem.
   - *Note: `@<refspec>` and `[<sub_folder>]` are ignored for local folders and files.*

### Classification Decision Table

Here's a quick reference for the virtual directory resolution rules:

| Rule # | Starts with `http(s)://`? | `<source>` Suffix | `@<refspec>` or `[<sub_folder>]` Present? | Resolved Variant |
| :--- | :--- | :--- | :--- | :--- |
| **1a** | Yes | `.zip` or `.tar.gz` | Either (refspec ignored) | **Remote Archive** |
| **1b** | Yes | `.git` | Either | **Git Repository** |
| **1c** | Yes | Any other suffix | Yes (at least one) | **Git Repository** |
| **1d** | Yes | Any other suffix | No (neither) | **Remote File** |
| **2a** | No | `.zip` or `.tar.gz` | Either (refspec ignored) | **Local Archive** |
| **2b** | No | Any other suffix | Either (both ignored) | **Local Folder / File** |


## How Each Source Type Resolves

For all remote and archive sources, Weaver creates a temporary directory under `$HOME/.weaver/vdir_cache/repo<random>` to store downloaded files, extracted archives, or cloned Git worktrees. This temporary directory is automatically removed when the command finishes (or when the `VirtualDirectory` handle is dropped).

### Local Folder or File

- **Resolution**: Uses the local path directly on disk without copying files or creating a temporary directory.
- **Supported targets**: Depending on the command or option, `<source>` can point to:
  - A local directory (such as a registry directory containing YAML definitions, a policy directory, or a templates root directory).
  - A single local file (such as a registry `manifest.yaml` / `registry_manifest.yaml` file, a standalone resolved registry file, a `.rego` policy file, or a `.json` / `.yaml` / `.jq` advice data file).

### Local and Remote Archives (`.zip`, `.tar.gz`)

- **Resolution**:
  - For **Local Archives**, Weaver opens the `.zip` or `.tar.gz` file from disk and extracts it into the temporary cache directory.
  - For **Remote Archives**, Weaver first downloads the archive over HTTP(S) into the temporary cache directory and then extracts it.
- **Top-level directory stripping**: Archive extraction is designed for release source archives (such as GitHub release `.zip` and `.tar.gz` archives), where all files are packaged inside a single top-level directory (for example, `semantic-conventions-1.27.0/model/...`). During extraction, Weaver **automatically strips the first path component** of every archive entry.
- **Sub-folder extraction (`[<sub_folder>]`)**:
  - When `[<sub_folder>]` is **omitted**, all entries beneath the stripped top-level archive folder are extracted directly into the virtual directory root.
  - When `[<sub_folder>]` is **specified** (for example, `archive.tar.gz[model]`), Weaver filters for entries under `<top-level-dir>/<sub_folder>/` and strips `<sub_folder>` as well, placing the contents of `<sub_folder>` at the root of the virtual directory.
- **Security safeguards**: Archive entries containing parent directory traversal (`..`) or root path components, as well as symbolic or hard links in `.tar.gz` archives, are skipped during extraction.

### Git Repositories (`http://`, `https://`)

- **Resolution**: Clones the repository over HTTP(S) into the temporary cache directory using Weaver's built-in pure-Rust Git client (`gix`).
- **Refspec behavior (`@<refspec>`)**:
  - **Omitted**: Performs a shallow clone (`--depth 1`) of the remote repository's default branch (`HEAD`).
  - **Branch or tag name** (e.g., `@main`, `@v1.27.0`): Fetches the matching branch (`refs/heads/<name>`) or tag (`refs/tags/<name>`) and checks out its worktree.
  - **Commit SHA**: Must be a **full 40-character hexadecimal SHA-1** (e.g., `@0123456789abcdef0123456789abcdef01234567`). Fetches the repository and checks out the tree for that exact commit. Short commit SHAs are not treated as commit object IDs.
- **Sub-folder selection (`[<sub_folder>]`)**:
  - If `[<sub_folder>]` is provided (for example, `[model]`), Weaver verifies that the sub-folder exists inside the checked-out worktree and sets the virtual directory root to `<worktree>/<sub_folder>`.

### Remote Files (`http://`, `https://`)

- **Resolution**: Downloads the single file over HTTP(S) into the temporary cache directory, preserving the filename from the URL path. The resulting virtual directory path points directly to the downloaded file on disk.
- **Common uses**:
  - Pointing to a published registry manifest (`manifest.yaml`) or resolved schema (`resolved.yaml`).
  - Loading a remote `.rego` policy file or advice data file.
- **Relative path resolution from manifests**: When a publication manifest is loaded from a remote file URL (for example, `https://example.com/schemas/my-registry/1.0.0/manifest.yaml`) and its `resolved_registry_uri` is a relative path (such as `resolved.yaml` or `./resolved.yaml`), Weaver resolves that relative path against the parent URL of the manifest (`https://example.com/schemas/my-registry/1.0.0/resolved.yaml`).

### GitHub Release Asset URLs

When a **Remote Archive** or **Remote File** URL matches GitHub's browser release download format:

```text
https://github.com/{owner}/{repo}/releases/download/{tag}/{filename}
```

Weaver automatically resolves the asset through the GitHub Releases API:

1. Queries `https://api.github.com/repos/{owner}/{repo}/releases/tags/{tag}` (caching the release metadata per `{owner}/{repo}/{tag}` for the duration of the process).
2. Finds the asset matching `{filename}`.
3. Downloads the asset via `https://api.github.com/repos/{owner}/{repo}/releases/assets/{id}` with `Accept: application/octet-stream`.

This allows standard browser-copied GitHub release links to work seamlessly with Bearer token authentication on private repositories (where direct browser download URLs would otherwise fail due to cross-domain redirects stripping `Authorization` headers).

---

## Authentication and Credentials

### HTTP(S) Authentication (`RemoteArchive` and `RemoteFile`)

For private HTTP(S) files, archives, and GitHub release assets, configure `[[auth]]` entries in [`.weaver.toml`](/crates/weaver_config/README.md):

```toml
[[auth]]
url_prefix = "https://github.com/my-org/"
token_env = "GITHUB_TOKEN"
```

- **Longest prefix wins**: Weaver matches the download URL against `url_prefix` and selects the most specific match. For GitHub release browser URLs (`https://github.com/{owner}/{repo}/releases/download/...`), `url_prefix` is matched against the original `https://github.com/...` URL even though Weaver downloads the asset via `https://api.github.com`.
- **HTTPS required**: `url_prefix` must use `https://`.
- **Token sources**: Each `[[auth]]` entry specifies exactly one of `token_env` (environment variable), `token_command` (shell command printing the token to stdout), or `token` (literal value). The resolved token is sent as `Authorization: Bearer <token>`.

### Git Credentials (`GitRepo`)

By default, Weaver clones Git repositories in **isolated (hermetic) mode**, ignoring system and user `~/.gitconfig` files so builds remain reproducible across environments.

To clone private HTTP(S) Git repositories using your local Git credential helpers or `~/.gitconfig` settings, pass the `--allow-git-credentials` flag on the CLI:

```bash
weaver registry check -r "https://github.com/my-org/private-semconv.git@main[model]" --allow-git-credentials
```

---

## Where Virtual Directory References Are Used

### 1. CLI Flags

| Flag | Commands | Description |
| :--- | :--- | :--- |
| `-r`, `--registry <REGISTRY>` | `check`, `generate`, `resolve`, `search`, `stats`, `update-markdown`, `json-schema`, `diff`, `emit`, `live-check`, `mcp`, `package` | Target semantic convention registry directory, archive, Git repo, or manifest file. |
| `--baseline-registry <BASELINE_REGISTRY>` | `check`, `diff` | Baseline semantic convention registry to compare against. |
| `-p`, `--policy <POLICIES>` | `check`, `generate`, `resolve`, `emit`, `live-check`, `package` | Rego policy directory, archive, Git repo, or `.rego` file (can be specified multiple times). |
| `-t`, `--templates <TEMPLATES>` | `generate`, `update-markdown` | Template directory, archive, or Git repo containing target subdirectories. |
| `--advice-policies <ADVICE_POLICIES>` | `live-check`, `mcp` | Rego advice policies directory, archive, Git repo, or `.rego` file. |
| `--advice-data <ADVICE_DATA>` | `live-check`, `mcp` | Supplemental jq/JSON/YAML advice data directory, archive, Git repo, or file. |

### 2. Project Configuration (`.weaver.toml`)

See [`crates/weaver_config/README.md`](/crates/weaver_config/README.md) for full `.weaver.toml` documentation.

| Configuration Key | Type | Description |
| :--- | :--- | :--- |
| `[registry].path` | `vdir` string | Default registry location (`-r` / `--registry`). |
| `[policy].paths` | Array of `vdir` strings | Default policy locations (`-p` / `--policy`). |
| `[resolve.schema_url_overrides]` | Map of `<schema_url> = "<vdir>"` | Redirects a dependency's `schema_url` to a local or remote `vdir` location during resolution. |
| `[generate].templates` | `vdir` string | Default templates location for `weaver registry generate`. |
| `[update-markdown].templates` | `vdir` string | Default templates location for `weaver registry update-markdown`. |
| `[live-check].advice_policies` | `vdir` string | Default advice policies location for `weaver registry live-check`. |
| `[live-check].advice_data` | `vdir` string | Default advice data location for `weaver registry live-check`. |
| `[mcp].advice_policies` | `vdir` string | Default advice policies location for `weaver registry mcp`. |
| `[mcp].advice_data` | `vdir` string | Default advice data location for `weaver registry mcp`. |

### 3. Registry Manifests (`manifest.yaml` / `registry_manifest.yaml`)

See [Semantic Conventions Schemas](/schemas/semconv-schemas.md) and [Define Your Own Telemetry Schema](/docs/define-your-own-telemetry-schema.md) for manifest details.

| Manifest Field | Manifest Type | Description |
| :--- | :--- | :--- |
| `dependencies[].registry_path` | Definition & Publication (`manifest/2.0` and legacy v1) | Optional `vdir` location of the dependency registry when it is not hosted directly at its `schema_url`. |
| `resolved_registry_uri` | Publication (`manifest/2.0`) | Location of the packaged resolved schema (`resolved.yaml`). Can be a relative path (resolved relative to the manifest's `vdir`) or a standalone `vdir` string. |
