param([switch]$Release,[switch]$Offline,[switch]$Tests)
$ErrorActionPreference='Stop'
$buweiRoot=Split-Path $PSScriptRoot -Parent
$buweiNativeFlags=@()
if($env:CARGO_ENCODED_RUSTFLAGS){$buweiNativeFlags=@($env:CARGO_ENCODED_RUSTFLAGS -split [char]31)}
if(!(Test-Path -LiteralPath (Join-Path $buweiRoot '.deps/octosense/.sources/makepad/Cargo.toml'))){throw '请先运行 python tools/bootstrap.py 获取固定的官方依赖。'}
$buweiRustHost=(& rustc -vV | Select-String '^host:').ToString()
if($buweiRustHost -match 'windows-gnu'){
 $buweiGccLibrary=(& gcc -print-libgcc-file-name).Trim()
 if($LASTEXITCODE -ne 0 -or !(Test-Path -LiteralPath $buweiGccLibrary)){throw 'GNU 构建需要 MinGW GCC。'}
 $buweiCompat=Join-Path $buweiRoot '.run/linker-compat'
 New-Item -ItemType Directory -Path $buweiCompat -Force | Out-Null
 Copy-Item -LiteralPath $buweiGccLibrary -Destination (Join-Path $buweiCompat 'libgcc_eh.a') -Force
 $buweiNativeFlags+=@('-L',('native='+[IO.Path]::GetDirectoryName($buweiGccLibrary)),'-L',('native='+$buweiCompat),'-C','link-arg=-Wl,--stack,16777216')
}
if($Release){
 $buweiNativeFlags+=@('--remap-path-prefix',($buweiRoot+'=/buwei'))
 foreach($buweiPrivateRoot in @($env:USERPROFILE,$env:CARGO_HOME,$env:RUSTUP_HOME)){
  if($buweiPrivateRoot){$buweiNativeFlags+=@('--remap-path-prefix',($buweiPrivateRoot+'=/toolchain'))}
 }
}
if($buweiNativeFlags.Count){$env:CARGO_ENCODED_RUSTFLAGS=$buweiNativeFlags -join [char]31}
$env:CARGO_BUILD_JOBS='3'
$buweiArguments=@($(if($Tests){'test'}else{'build'}),'--manifest-path','native/Cargo.toml','--locked','--features','full-host')
if($Tests){$buweiArguments+='--workspace'}
if($Release){$buweiArguments+='--release'}
if($Offline){$buweiArguments+='--offline'}
Push-Location $buweiRoot
try {
 $buweiProofStart=Join-Path $buweiRoot '.run/build-start.json'
 if(!$Tests){python tools/build_proof.py --start $buweiProofStart;if($LASTEXITCODE -ne 0){throw '构建来源不可核实。'}}
 & cargo @buweiArguments; if($LASTEXITCODE -ne 0){throw '构建或测试失败；保留现有数据。'}
 if(!$Tests){
  $buweiTargetRoot=if($env:CARGO_TARGET_DIR){$env:CARGO_TARGET_DIR}else{Join-Path $buweiRoot 'native/target'}
  $buweiProfile=if($Release){'release'}else{'debug'}
  $buweiBinary=Join-Path $buweiTargetRoot ($buweiProfile+'/buwei-rinx-dual-host.exe')
  python tools/pe_stack.py $buweiBinary
  if($LASTEXITCODE -ne 0){throw 'Windows 可执行文件栈设置核验失败，禁止打包。'}
  python tools/build_proof.py --finish $buweiProofStart --binary $buweiBinary --output ($buweiBinary+'.build.json')
  if($LASTEXITCODE -ne 0){throw '构建期间来源发生变化，禁止打包。'}
 }
}
finally {Pop-Location}
