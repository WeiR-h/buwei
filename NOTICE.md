# Third party notices

BuWei original application and action-receipts code Copyright 2026 WeiR-h,
Apache-2.0. Team: 生生不息. Support: repository Issues.

BuWei's Chinese interface uses unmodified Source Han Sans SC Regular and Bold,
Copyright Adobe, under the SIL Open Font License 1.1. The license is in
licenses/SourceHanSans-LICENSE.txt; original URLs and SHA-256 values are in
native/resources/fonts/sources.json.

Official components retain their authors' copyright and licenses. All Rust
sources are pinned in native/Cargo.lock. Frameworks are pinned in
 dependencies.lock.json. Bootstrap preserves upstream license files.

- OctoSense desktop RC6: f9f6fea90b52182790cb2641fd0145c425af4d84
- Makepad: d653bc4493619ab979ccff165cf48c0c63b77d19
- OctoScript: 2e37d9e657a246f16718d9a475e167ccd2d5b5fa
- OctoScript Makepad: 6cf2d90aa33ff4ae603f49c7557f3ecf986207f0
- Rinx: 4b89097d8791a7190d01de1c576979c93df0013d
- App Hub: 386359153ed6377b5f047bd9ed2767a6a2ac2bc7
- octos: b0759a57719fd35b3a2da1c5d969bc67538ed516
- Matrix SDK: 6892cb217ae4a886571e928c8efcccfbec5490a6

Official Makepad overlays are locked in OctoSense/runtime-patches.lock.json.
BuWei adds the separately recorded patches/makepad-unicode-smallvec.patch.
BuWei also adds patches/makepad-windows-warp.patch: hardware device creation
failure falls back to the Windows WARP renderer. Both patches have fixed
SHA-256 values and exact modified-file verification in dependencies.lock.json.
The separately reviewed patches/octosense-native-system-tools.patch exposes
named tools of package-linked native modules to the system chat through the
existing approval router and AI bus. It does not grant Matrix execution rights.
Its hash and modified files are pinned and verified by the bootstrap tool.
This build does not imply official endorsement or competition acceptance.
Binary packages must include upstream license texts and a dependency license
inventory. The acceptance record states actual package validation results.

Windows 可执行文件在构建后核验并统一主线程栈预留为 16 MiB。
工具只修改未签名 PE 的栈预留和校验和，确认代码与资源不变。
该构建修正不改变身份、授权或业务逻辑；运行包仍必须通过实际启动。
