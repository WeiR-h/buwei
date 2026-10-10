param([switch]$Offline)
$ErrorActionPreference='Stop'
$buweiRoot=Split-Path $PSScriptRoot -Parent
$buweiPins=Get-Content -LiteralPath (Join-Path $buweiRoot 'dependencies.lock.json') -Raw | ConvertFrom-Json
$buweiKernelSource=Join-Path $buweiRoot ('.deps/octosense/'+$buweiPins.runtime_sources.octos.directory)
$buweiRevision=(& git -C $buweiKernelSource rev-parse HEAD).Trim()
if($LASTEXITCODE -ne 0 -or $buweiRevision -ne $buweiPins.runtime_sources.octos.commit){throw 'Kernel source revision differs; existing runtime preserved.'}
if(& git -C $buweiKernelSource status --porcelain){throw 'Modified official kernel source; existing runtime preserved.'}
$buweiTarget=if($env:CARGO_TARGET_DIR){$env:CARGO_TARGET_DIR}else{Join-Path $buweiRoot 'native/target'}
$buweiArguments=@('build','--manifest-path',(Join-Path $buweiKernelSource 'Cargo.toml'),'--locked','--release','--target-dir',$buweiTarget,'-p','octos-cli','--bin','octos','--no-default-features','--features','api,git,ast')
if($Offline){$buweiArguments+='--offline'}
& cargo @buweiArguments
if($LASTEXITCODE -ne 0){throw 'Official desktop kernel build failed; existing runtime preserved.'}
$buweiKernel=Join-Path $buweiTarget 'release/octos.exe'
$buweiStage=Join-Path $buweiTarget 'release'
& python (Join-Path $buweiRoot '.deps/octosense/tools/kernel-artifact.py') --host --kernel $buweiKernel --lock (Join-Path $buweiRoot '.deps/octosense/Cargo.lock') --stage $buweiStage
if($LASTEXITCODE -ne 0){throw 'Official kernel staging/revision verification failed.'}
