# BuWei

BuWei helps community organizers follow sign-ups, waiting lists, invitations and last-minute replacements for sports, outdoor activities, cultural events, learning groups, meals and volunteering. Organizers and members save their goals; BuWei notices relevant changes, prepares the next step and follows verified results.

Author: **WeiR-h** · Team: **生生不息** · Apache-2.0

Participants use their own Rinx identities. Verified server receipts distinguish an invitation from an accepted place, and preserve operation IDs for recovery.

- Manage up to five dated activities, each with 30 places and 30 active registrations.
- Search 18 activity templates with editable agendas and preparation lists, including running, hiking, cycling, ball games, reading, meals, workshops and family activities, or choose a custom activity. The same types work in goals, recommendations, discovery and exports. [Activity guide](docs/ACTIVITY-TYPES.md).
- Accept parties of 1–8 people together, including activities that cross midnight.
- Share an activity card through Rinx and let members sign up using their own identities.
- Approve activity-specific replacement rules to invite eligible waiting members automatically while the app is open.
- Save personal goals and explicitly confirmed preferences; receive suggestions about shortages, matching activities, pending invitations and schedule conflicts.
- Follow suggestions through persistent tasks. Editing or pausing a goal invalidates its unexecuted previews while preserving existing receipts.
- Use AI beside each task for editable activity drafts, sign-up clarification and summaries grounded in verified records.

[Download v0.2.13 activity preview](https://github.com/WeiR-h/buwei/releases/tag/v0.2.13) · [Stable release](https://github.com/WeiR-h/buwei/releases/latest) · [Chinese guide](docs/FIRST-RUN.md) · [Privacy](docs/PRIVACY.md) · [Support](https://github.com/WeiR-h/buwei/issues)

**v0.2.13 activity preview** uses the official desktop RC6 host and adds 18 searchable templates with editable agendas, preparation lists and participation notes. The shared catalog connects planning, recommendations, discovery and CSV exports. Each account can organize its own activities and join other people's activities.

Organizers can describe an activity, edit its plan, confirm creation, and separately confirm community publication. Members select a published activity, request room access, and confirm their own registration. CSV exports use a verified organizer snapshot and protect text from spreadsheet formula interpretation. See the [community guide](docs/COMMUNITY.md).

BuWei uses pinned official OctoSense desktop RC6 and Rinx 1.1.0 sources and ships as a native Windows host extension with organizer and participant entry points. Participants sign up and reply with their own Rinx identities; the organizer verifies results before displaying a confirmed place.

Optional Windows tray duty continues synchronization while the computer and application run with valid authorization. Explicit feedback updates only the chosen scope. Recurring activity drafts require fresh confirmation of their date, location and capacity. [Goals and assistance](docs/INTENTIONS.md) · [Submission and historical versions](docs/SUBMISSION.md).

[Watch the 60-second continuous native recording](https://github.com/WeiR-h/buwei/releases/download/v0.2.12/BuWei-v0.2.12-demo.mp4): two real local Rinx accounts complete a natural-language sign-up, rule-based invitation, personal acceptance and final organizer verification. Capture preserves elapsed time and covers private identifiers. [Planning, community publication and export walkthrough (v0.2.11)](https://github.com/WeiR-h/buwei/releases/download/v0.2.11/BuWei-v0.2.11-Demo.mp4). Runtime requirements: Windows 10/11 x64 and the Microsoft Visual C++ v14 x64 runtime.

## Three steps to start

1. Extract the complete runtime package, open the BuWei entry point and sign in to Rinx. The organizer and participant launchers keep separate profiles for testing two accounts.
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

[Reproduce production rules, journal performance and real SDK checks](docs/VERIFICATION.md). The portable Windows/Linux checks need no account or model credentials; live-service evidence is labelled separately from independent user trials and judging.
