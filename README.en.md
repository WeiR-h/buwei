# BuWei

BuWei helps community organizers follow sign-ups, waiting lists, invitations and last-minute replacements for badminton, board games and reading groups. Organizers and members save their goals; BuWei notices relevant changes, prepares the next step and follows verified results.

Author: **WeiR-h** · Team: **生生不息** · Apache-2.0

Participants use their own Rinx identities. Verified server receipts distinguish an invitation from an accepted place, and preserve operation IDs for recovery.

- Manage up to five dated activities, each with 30 places and 30 active registrations.
- Accept parties of 1–8 people together, including activities that cross midnight.
- Share an activity card through Rinx and let members sign up using their own identities.
- Approve activity-specific replacement rules to invite eligible waiting members automatically while the app is open.
- Save personal goals and explicitly confirmed preferences; receive suggestions about shortages, matching activities, pending invitations and schedule conflicts.
- Follow suggestions through persistent tasks. Editing or pausing a goal invalidates its unexecuted previews while preserving existing receipts.
- Use AI beside each task for editable activity drafts, sign-up clarification and summaries grounded in verified records.

[Download for Windows](https://github.com/WeiR-h/buwei/releases/latest) · [Chinese guide](docs/FIRST-RUN.md) · [Privacy](docs/PRIVACY.md) · [Support](https://github.com/WeiR-h/buwei/issues)

The current source version is **v0.2.3**. Runtime downloads and walkthroughs are provided with the corresponding fixed Release.

BuWei uses pinned official OctoSense and Rinx 1.1.0 sources and ships as a native Windows host extension with organizer and participant entry points. Participants sign up and reply with their own Rinx identities; the organizer verifies results before displaying a confirmed place.

Optional Windows tray duty continues synchronization while the computer and application run with valid authorization. Explicit feedback updates only the chosen scope. Recurring activity drafts require fresh confirmation of their date, location and capacity. [Goals and assistance](docs/INTENTIONS.md) · [Submission and historical versions](docs/SUBMISSION.md).

Each Release includes a three-minute walkthrough made from reviewed captures of the actual native interface. Runtime requirements: Windows 10/11 x64 and the Microsoft Visual C++ v14 x64 runtime.

## Three steps to start

1. Extract the complete runtime package, open the organizer or participant entry point and sign in to Rinx.
2. Open BuWei, verify your account and approve the permissions you need.
3. Save your goal. Organizers create and share an activity; participants join from its card and confirm their own sign-up or invitation reply.

## Build and support

Build on Windows x64 with Git, Python 3.12+, Rust 1.98.0, Visual Studio C++ Desktop tools and the Windows SDK:

```powershell
python tools/bootstrap.py
./tools/Build.ps1 -Tests
./tools/Build.ps1 -Release
```

Pinned official dependencies, source, tests and required host patches are included in this repository. AI configuration belongs to the native host and remains on the user's device.
