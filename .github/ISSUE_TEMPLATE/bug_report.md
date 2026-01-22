---
name: Bug Report
about: Report a bug or issue with the Momoi
title: '[BUG] '
labels: bug
assignees: ''
---

## Bug Description

A clear and concise description of the bug.

## Steps to Reproduce

1. Start the daemon with: `...`
2. Run command: `...`
3. Observe: `...`

## Expected Behavior

What you expected to happen.

## Actual Behavior

What actually happened.

## System Information

**Operating System:**
- Distribution: (e.g., NixOS, Arch Linux, Ubuntu)
- Version: (e.g., 23.11, Rolling)

**Wayland Compositor:**
- Name: (e.g., Sway, Hyprland, River)
- Version: `sway --version` or equivalent

**GPU Information:**
```bash
lspci | grep VGA
# Paste output here
```

**Vulkan Support:**
```bash
vulkaninfo | head -20
# Paste output here
```

**Daemon Version:**
```bash
momoi --version
# Paste output here
```

## Configuration

**Config file** (if relevant):
```toml
# Paste relevant parts of your config.toml here
```

**Command used:**
```bash
# Exact command that triggered the bug
wwctl ...
```

## Logs

**Daemon logs** (with debug logging enabled):
```bash
RUST_LOG=debug momoi 2>&1 | tee daemon.log
# Paste relevant log lines here (last 50 lines usually sufficient)
```

## Additional Context

Add any other context, screenshots, or information about the problem here.

## Workarounds

If you found a workaround, please share it here to help others.
