# BuWei

BuWei helps community organizers manage sign-ups, waiting lists, invitations and last-minute replacements for badminton, board games and reading groups.

Author: **WeiR-h** · Team: **生生不息** · Apache-2.0

Participants use their own Rinx identities. Verified server receipts distinguish an invitation from an accepted place, and preserve operation IDs for recovery.

[Download for Windows](https://github.com/WeiR-h/buwei/releases/latest) · [Chinese guide](docs/FIRST-RUN.md) · [Privacy](docs/PRIVACY.md) · [Support](https://github.com/WeiR-h/buwei/issues)

Build on Windows x64 with Git, Python 3.12+, Rust 1.98.0, Visual Studio C++ Desktop tools and the Windows SDK:

```powershell
python tools/bootstrap.py
./tools/Build.ps1 -Tests
./tools/Build.ps1
```

Pinned official dependencies, source, tests and required host patches are included in this repository. AI configuration belongs to the native host and remains on the user's device.
