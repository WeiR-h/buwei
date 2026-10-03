# BuWei

Windows activity waitlist extension in the official OctoSense / Rinx host.
Author: WeiR-h. Team: 生生不息. License: Apache-2.0.

One organizer manages one activity for up to 30 participants. Each participant
uses their own Rinx identity. Model suggestions require review; deterministic
queue, availability and capacity rules decide eligibility. Invitation delivery
and personal acceptance are separate states.

**Current source: v0.1.0.** [Downloads](https://github.com/WeiR-h/buwei/releases).
85 Rust and 7 release-tool checks, five real dual-account loops, decline,
expiration, recovery and network interruption checks passed. A clean Windows
build and a separate fresh Windows runtime both passed. The stable tag requires
public-download verification. See [acceptance](docs/ACCEPTANCE.md) for evidence.

## Build

Windows x64, Git, Python 3.12+, Rust 1.98.0, and Visual Studio
with Desktop development with C++ and Windows SDK for MSVC builds. The public
package uses MSVC; provenance records the compiler tools. GNU / MinGW also has
local core and real SDK checks, with GCC and Binutils recorded. First build downloads
fixed upstream dependencies.
Keys and signed-in profiles are not needed to build.

```powershell
git clone https://github.com/WeiR-h/buwei.git
cd buwei
python tools/bootstrap.py
./tools/Build.ps1 -Tests
./tools/Build.ps1
```

Executable: `native/target/debug/buwei-rinx-dual-host.exe`.
[First run](docs/FIRST-RUN.md) explains isolated profiles and Rinx login.
Uncertain sends retain their operation ID and reconcile without blind retries.
Sources include `action-receipts` and business adapters. Dependencies are pinned
in `native/Cargo.lock` and `dependencies.lock.json`. Private data is ignored.
PROVENANCE.json records source and binary-build commits separately. Documentation
updates preserve the tested native source, dependency hashes and executable.
No credentials or business data are included. Use repository Issues for support.

[Privacy](docs/PRIVACY.md) · [Notices](NOTICE.md) ·
[Support](https://github.com/WeiR-h/buwei/issues)
