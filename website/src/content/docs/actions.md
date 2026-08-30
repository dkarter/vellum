---
title: Actions
description: Run safe native operations on the selected Vellum item.
---

Actions have a unique name, display label, optional icon and description, one command form, and an `on_success` policy.

```toml
[actions]
default = "open"
menu = "ctrl-a"

[[actions.items]]
name = "open"
label = "Open item"
command = ["my-tool", "open", "$agent_id"]
on_success = "exit"

[[actions.items]]
name = "remove"
label = "Remove item"
key = "ctrl-r"
command = ["my-tool", "remove", "--id", "$agent_id"]
when = [{ field = "removable", equals = true }]
on_success = "refresh"
```

## Direct execution

`command` is an argv array executed directly, never through a shell. An argument consisting entirely of a field expression is replaced by that scalar field as one literal argument. Spaces and shell metacharacters cannot become shell syntax. Missing, null, array, or object fields fail safely and keep Vellum open.

`cwd` can be a literal path or a selected scalar field:

```toml
cwd = "$checkout_path"
command = ["gh", "pr", "view", "--web"]
```

## Conditions and availability

Every `when` condition must match. Use `equals` for an exact scalar or `is_set = true` for a present, non-null field.

An `availability` probe can hide an action until a side-effect-free argv command succeeds:

```toml
availability = { command = ["gh", "pr", "view", "--json", "number"], cwd = "$checkout_path", cache_ms = 30000, timeout_ms = 5000 }
```

Probes run in the background with stdin, stdout, and stderr disconnected. Results share a cache by interpolated command, working directory, and timeout policy.

## Dispatch and outcomes

Without `actions.default`, Enter prints `item.value`. A default action replaces that behavior. `key` invokes an action directly; `actions.menu` opens a fuzzy-searchable action list and defaults to Ctrl-A. Set it to `false` to disable the menu.

- `on_success = "exit"` closes without selection output.
- `on_success = "refresh"` reruns the source and keeps the query and nearest useful selection.

Actions currently run synchronously. Failed commands leave Vellum open and show stderr in the footer.

Use `shell = "..."` only for intentional shell syntax. Shell actions do not interpolate item fields. All action processes are non-interactive: stdout is discarded, stderr is captured, and they do not receive the terminal.
