# Executable Registry Plan

## Purpose

Create a tool that manages executable commands from files stored in different locations.

The tool will maintain one common directory in `$PATH`, such as `~/.local/bin`. It will create and remove symbolic links in that directory from a declarative registry.

The tool will provide both a CLI and a TUI.

## Goals

- Register an executable from any filesystem location.
- Assign a command name that differs from the source filename.
- Enable or disable each registered command.
- Create, update, and remove managed symbolic links.
- Detect missing targets, broken links, incorrect links, and name conflicts.
- Find executables that moved and update their registry entries.
- Preview changes before the tool applies them.
- Keep the registry easy to inspect and store with dotfiles.
- Make commands available to all shells and non-interactive processes.

## Non-goals

- Install third-party packages or language runtimes.
- Replace mise as a tool-version manager.
- Replace mise tasks that define workflows, environments, dependencies, or complex commands.
- Manage general dotfiles like GNU Stow.
- Treat every source directory as a package.

## Recommended technology

Use Rust and produce one executable.

Suggested libraries:

- Clap for CLI argument parsing.
- Ratatui and Crossterm for the TUI.
- Serde and a TOML library for registry data.
- A filesystem-locking library for safe registry updates.

Rust provides fast startup, one distributable binary, strong filesystem support, and explicit types for reconciliation operations.

## Architecture

Keep the core behavior independent of the CLI and TUI.

```text
┌────────────┐       ┌──────────────────┐
│ CLI        │──────▶│                  │
└────────────┘       │ Registry and     │
                     │ reconciliation   │───▶ filesystem
┌────────────┐       │ core             │
│ TUI        │──────▶│                  │
└────────────┘       └──────────────────┘
```

Suggested source layout:

```text
src/
├── main.rs
├── cli.rs
├── tui.rs
├── model.rs
├── registry.rs
├── reconcile.rs
└── discovery.rs
```

### Core modules

- `model`: Registry entries, health states, conflicts, and planned operations.
- `registry`: Load, validate, lock, and atomically save the TOML registry.
- `reconcile`: Compare desired state with actual filesystem state.
- `discovery`: Search for moved executable targets and rank candidates.
- `cli`: Parse commands and present machine-friendly or human-friendly output.
- `tui`: Present interactive views and call the same core operations as the CLI.

The TUI must not contain separate filesystem-management logic.

## Data model

Use TOML as the source of truth.

Default locations:

```text
Registry:  ~/.config/binreg/registry.toml
Commands:  ~/.local/bin/
```

Example registry:

```toml
version = 1
bin_dir = "~/.local/bin"

[roots]
software = "~/Software"
devbox = "~/Software/DevBox"

[[command]]
name = "w"
root = "devbox"
path = "tools/util/w"
enabled = true
fingerprint = "sha256:..."

[[command]]
name = "gitview"
root = "software"
path = "DevApps/Tauri/gitview/src-tauri/target/release/gitview"
enabled = false
```

Root-relative paths make directory moves easier to manage. The tool can change one root instead of changing every related entry.

Each command can contain:

- Command name.
- Absolute target or root-relative target.
- Enabled state.
- Optional content fingerprint.
- Optional Git repository identity and repository-relative path.
- Optional notes or tags.

## CLI

Initial command design:

```console
binreg add <target>
binreg add --name <name> <target>
binreg remove <name>
binreg enable <name>
binreg disable <name>
binreg list
binreg check
binreg plan
binreg sync
binreg repair [name]
binreg find <name> [search-root]
binreg tui
```

Running `binreg` without a subcommand can open the TUI when standard input and output are terminals. Scripts must use explicit subcommands.

### Command behavior

#### `add`

- Resolve the target to a stable absolute or root-relative path.
- Verify that the target exists and is executable.
- Derive the command name from the filename unless `--name` is present.
- Detect registry and destination conflicts.
- Add the enabled registry entry.
- Create the link unless `--no-sync` is present.

#### `remove`

- Remove the registry entry.
- Remove its managed symbolic link.
- Never remove the target executable.

#### `enable` and `disable`

- Change the desired state.
- Create or remove the managed link.

#### `check`

Report:

- Missing targets.
- Targets that are not executable.
- Missing managed links.
- Broken symbolic links.
- Links that point to incorrect targets.
- Duplicate command names.
- Destination conflicts.
- Unmanaged files in the common bin directory.

`check` must not modify files.

#### `plan`

Create a deterministic list of proposed operations without applying them.

Possible operations include:

- Create link.
- Replace incorrect link.
- Remove link for a disabled entry.
- Report an unmanaged destination conflict.
- Report a missing source target.

#### `sync`

- Generate a plan.
- Refuse unsafe operations by default.
- Apply safe operations.
- Require explicit confirmation or `--force` before replacing unmanaged destinations.

#### `find`

Search for a moved target by using:

1. Original filename.
2. Executable permission.
3. Stored content fingerprint.
4. Git repository identity and repository-relative path.
5. Other stable file metadata where available.

If there is one high-confidence result, the tool can propose an automatic repair. If there are multiple results, the CLI will print candidates and the TUI will provide a selection view.

## TUI

The TUI is a thin interactive client over the core modules.

Example main view:

```text
 Executable Registry                       18 enabled · 2 broken

 Filter: weather_

 [✓] w          ~/Software/DevBox/tools/util/w             linked
 [ ] weather    ~/Software/Test/weather/target/weather     disabled
 [!] forecast   ~/Software/Old/forecast                    missing

 Space toggle   a add   r repair   s sync   / filter   q quit
```

### Views

#### Registry view

- List all entries.
- Filter by command name, tag, source root, or health state.
- Enable or disable entries.
- Add, rename, and remove entries.
- Reveal a target in Finder.
- Copy a target path.

#### Health view

- Show missing targets.
- Show broken or incorrect links.
- Show non-executable targets.
- Show name and destination conflicts.
- Show unmanaged files in the bin directory.

#### Repair view

- Search for moved targets.
- Rank possible replacements.
- Show why each candidate matched.
- Let the user select a target.
- Preview the registry and link changes.

#### Sync preview

- Show links that will be created.
- Show links that will be changed.
- Show links that will be removed.
- Separate safe operations from conflicts.
- Require confirmation before application.

### Responsiveness

Filesystem scans must not block input or screen rendering. Run slow discovery operations in background workers and send progress events to the TUI.

## Reconciliation and safety

The registry defines desired state. The common bin directory is generated state.

Use a separate `plan` and `apply` process:

1. Load and validate the registry.
2. Inspect the target files and destination directory.
3. Build a complete operation plan.
4. Present the plan through the CLI or TUI.
5. Apply approved operations.
6. Inspect the result and report failures.

Safety rules:

- Never delete a target executable.
- Never replace a regular file by default.
- Never replace an unmanaged symbolic link by default.
- Mark managed links by registry ownership instead of assuming that all links in the directory are managed.
- Use temporary files and atomic renames for registry updates.
- Lock the registry during writes.
- Keep a backup of the prior registry version.
- Make `sync` idempotent.

## Relationship with the current setup

Use each mechanism for one clear concern:

- mise `[tools]`: Install runtimes and third-party tools.
- mise `[tasks]`: Define workflows, environments, dependencies, and complex commands.
- `binreg`: Publish standalone local executables through one `$PATH` directory.
- Zsh aliases: Define shell-only abbreviations.

Direct mise task wrappers such as this are candidates for migration:

```toml
run = "/Users/kenbanks/Software/DevBox/tools/util/w"
```

Complex tasks, such as policy workflows or commands that require mise-managed tools and environment values, should remain mise tasks.

The generated Zsh aliases can remain during migration. Remove each alias only after its command is available through `binreg`.

## Delivery phases

### Phase 1: Core registry

- Create the Rust project.
- Define and validate the TOML schema.
- Implement root expansion and target resolution.
- Implement atomic registry writes and locking.
- Add unit tests for registry parsing and validation.

### Phase 2: Reconciliation engine

- Inspect targets and destination links.
- Define health states and planned operations.
- Implement `check`, `plan`, and `sync`.
- Add tests with temporary filesystem trees.
- Confirm that repeated synchronization makes no further changes.

### Phase 3: Basic CLI

- Implement `add`, `remove`, `enable`, `disable`, and `list`.
- Add human-readable output.
- Add structured JSON output for scripting where useful.
- Define stable exit codes for healthy, degraded, conflict, and operational-error states.

### Phase 4: Initial TUI

- Implement the registry list and health indicators.
- Add filtering and keyboard navigation.
- Add enable and disable actions.
- Add a sync-preview dialog.
- Add clear error and confirmation dialogs.

### Phase 5: Discovery and repair

- Store optional target fingerprints.
- Search configured roots for moved executables.
- Rank and explain candidates.
- Add CLI and TUI repair flows.
- Run scans in background workers.

### Phase 6: Migration

- Set `~/.local/bin` as the managed directory.
- Ensure that it occurs once in `$PATH`.
- Inventory direct executable wrappers in the global `mise.toml`.
- Register selected executables.
- Compare command resolution before removing wrappers.
- Keep complex mise tasks unchanged.
- Retire the existing `symlink-bin` script after migration is complete.

## Validation criteria

The first stable version is complete when:

- CLI and TUI operations produce the same registry and filesystem results.
- Disabled entries do not have managed links.
- `check` detects each supported fault state without changing files.
- `sync` is idempotent.
- The tool does not overwrite unmanaged destinations without explicit approval.
- An interrupted registry write does not corrupt the prior registry.
- A moved executable can be found and repaired through the TUI.
- Commands work outside interactive Zsh sessions.
