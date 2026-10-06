# BuWei

BuWei helps community organizers manage sign-ups, waiting lists, invitations and last-minute replacements for badminton, board games and reading groups.

Author: **WeiR-h** · Team: **生生不息** · Apache-2.0

Participants use their own Rinx identities. Verified server receipts distinguish an invitation from an accepted place, and preserve operation IDs for recovery.

- Manage up to five dated activities, each with 30 places and 30 active registrations.
- Accept parties of 1–8 people together, including activities that cross midnight.
- Share an activity card through Rinx and let members sign up using their own identities.
- Approve activity-specific replacement rules to invite eligible waiting members automatically while the app is open.
- Use AI beside each task for editable activity drafts, sign-up clarification and summaries grounded in verified records.

[Download for Windows](https://github.com/WeiR-h/buwei/releases/latest) · [Chinese guide](docs/FIRST-RUN.md) · [Privacy](docs/PRIVACY.md) · [Support](https://github.com/WeiR-h/buwei/issues)

The current source preview adds private goals and explicitly confirmed preferences. Verified changes surface activity shortages, matching opportunities, pending invitations and time conflicts. Suggestions connect to persistent tasks, optional Windows tray duty and recurring activity drafts; personal feedback updates only the chosen scope. Published downloads follow the version stated in each Release. [Goals and assistance](docs/INTENTIONS.md).

Build on Windows x64 with Git, Python 3.12+, Rust 1.98.0, Visual Studio C++ Desktop tools and the Windows SDK:

```powershell
python tools/bootstrap.py
./tools/Build.ps1 -Tests
./tools/Build.ps1
```

Pinned official dependencies, source, tests and required host patches are included in this repository. AI configuration belongs to the native host and remains on the user's device.
