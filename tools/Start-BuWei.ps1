param(
 [ValidateSet('organizer','participant')][string]$Role='organizer',
 [string]$ProfileDirectory,
 [string]$Executable
)
$ErrorActionPreference='Stop'
$buweiRoot=Split-Path $PSScriptRoot -Parent
if(!$ProfileDirectory){$ProfileDirectory=Join-Path $env:LOCALAPPDATA ('BuWei/preview-'+$Role)}
if(!$Executable){$Executable=Join-Path $buweiRoot 'native/target/debug/buwei-rinx-dual-host.exe'}
if(!(Test-Path -LiteralPath $Executable)){throw '尚未找到补位程序，请先构建或使用完整运行包。'}
New-Item -ItemType Directory -Path $ProfileDirectory -Force | Out-Null
$buweiProfile=(Resolve-Path -LiteralPath $ProfileDirectory).Path
if(Test-Path -LiteralPath (Join-Path $buweiProfile 'migration-failed.local.json')){throw '迁移未通过验证，请使用保留的旧版入口。'}
$buweiLog=Join-Path $buweiProfile 'logs'
New-Item -ItemType Directory -Path $buweiLog -Force | Out-Null
foreach($buweiName in @('MAKEPAD_HIDE_WINDOWS','MAKEPAD_REMOTE','MAKEPAD_WM_TEST_APP','RINX_DATA_DIR','ROBRIX_DATA_DIR','CARGO_MANIFEST_DIR')){Remove-Item ('Env:'+$buweiName) -ErrorAction SilentlyContinue}
$env:BUWEI_PROFILE=$Role
$buweiTag='buwei-'+$Role+'-'+(Get-Date -Format 'yyyyMMdd-HHmmss-fff')
$buweiChild=Start-Process -FilePath $Executable -WorkingDirectory (Split-Path $Executable -Parent) -ArgumentList @(('"'+$buweiProfile+'"'),'--gui','--official-rinx') -WindowStyle Normal -PassThru -RedirectStandardOutput (Join-Path $buweiLog ($buweiTag+'.log')) -RedirectStandardError (Join-Path $buweiLog ($buweiTag+'.err.log'))
$buweiChild.WaitForExit()
if($buweiChild.ExitCode -ne 0){throw '启动未成功，资料和日志已保留。'}
