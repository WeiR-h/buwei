param([switch]$Release,[switch]$Offline,[switch]$Tests)
$ErrorActionPreference='Stop'
$buweiRoot=Split-Path $PSScriptRoot -Parent
if(!(Test-Path -LiteralPath (Join-Path $buweiRoot '.deps/octosense/.sources/makepad/Cargo.toml'))){throw '请先运行 python tools/bootstrap.py 获取固定的官方依赖。'}
$buweiRustHost=(& rustc -vV | Select-String '^host:').ToString()
if($buweiRustHost -match 'windows-gnu'){
 $buweiGccLibrary=(& gcc -print-libgcc-file-name).Trim()
 if($LASTEXITCODE -ne 0 -or !(Test-Path -LiteralPath $buweiGccLibrary)){throw 'GNU 构建需要 MinGW GCC。'}
 $buweiCompat=Join-Path $buweiRoot '.run/linker-compat'
 New-Item -ItemType Directory -Path $buweiCompat -Force | Out-Null
 Copy-Item -LiteralPath $buweiGccLibrary -Destination (Join-Path $buweiCompat 'libgcc_eh.a') -Force
 $buweiNativeFlags=@('-L',('native='+[IO.Path]::GetDirectoryName($buweiGccLibrary)),'-L',('native='+$buweiCompat),'-C','link-arg=-Wl,--stack,16777216')
 $env:CARGO_ENCODED_RUSTFLAGS=$buweiNativeFlags -join [char]31
}
$env:CARGO_BUILD_JOBS='3'
$buweiArguments=@($(if($Tests){'test'}else{'build'}),'--manifest-path','native/Cargo.toml','--locked','--features','full-host')
if($Tests){$buweiArguments+='--workspace'}
if($Release){$buweiArguments+='--release'}
if($Offline){$buweiArguments+='--offline'}
Push-Location $buweiRoot
try { & cargo @buweiArguments; if($LASTEXITCODE -ne 0){throw '构建或测试失败；保留现有数据。'} }
finally {Pop-Location}
