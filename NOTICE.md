# Third party notices

BuWei original application and action-receipts code Copyright 2026 WeiR-h,
Apache-2.0. Team: 生生不息. Support: repository Issues.

Official components retain their authors' copyright and licenses. All Rust
sources are pinned in native/Cargo.lock. Frameworks are pinned in
 dependencies.lock.json. Bootstrap preserves upstream license files.

- OctoSense: ad0d738bd1c10b735af6b34e11f5826623b6a73b
- Makepad: 4fdcfccc127b700f1fc01aa1a5488af938dd7f3d
- OctoScript: 68f6a9df55692b5d8ef8873a12721e279a3f40d6
- OctoScript Makepad: b33f494b963759088edf5785fc626cb8593818ee
- Rinx: 68afcf796d303aaf646eeb832d65c450a56c92b5
- App Hub: e8601b80ce104db2e48208094714bdcffdce6b5a
- octos: 5e7577f0cb92cf618b965745434b0200e2b82865
- Matrix SDK: 6892cb217ae4a886571e928c8efcccfbec5490a6

Official Makepad overlays are locked in OctoSense/runtime-patches.lock.json.
BuWei adds the separately recorded patches/makepad-unicode-smallvec.patch.
BuWei also adds patches/makepad-windows-warp.patch: hardware device creation
failure falls back to the Windows WARP renderer. Both patches have fixed
SHA-256 values and exact modified-file verification in dependencies.lock.json.
This build does not imply official endorsement or competition acceptance.
Binary packages must include upstream license texts and a dependency license
inventory. The acceptance record states actual package validation results.

Windows 可执行文件在构建后核验并统一主线程栈预留为 16 MiB。
工具只修改未签名 PE 的栈预留和校验和，逐字节确认代码与资源不变。
该构建修正不改变身份、授权或业务逻辑；运行包仍必须通过实际启动。
