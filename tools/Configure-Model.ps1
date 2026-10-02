param([Parameter(Mandatory=$true)][string]$ProfileDirectory)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Security
$buweiPrivateRoot=[IO.Path]::GetFullPath($ProfileDirectory)
$buweiForm=New-Object Windows.Forms.Form
$buweiForm.Text='补位 · MiniMax 本机配置'
$buweiForm.ClientSize=New-Object Drawing.Size(640,220)
$buweiForm.StartPosition='CenterScreen'
$buweiLabel=New-Object Windows.Forms.Label
$buweiLabel.Text='填写 minimax.cn 的普通 API key，仅加密保存在当前 Windows 用户下。'
$buweiLabel.Location=New-Object Drawing.Point(20,20)
$buweiLabel.Size=New-Object Drawing.Size(600,30)
$buweiInput=New-Object Windows.Forms.TextBox
$buweiInput.UseSystemPasswordChar=$true
$buweiInput.Location=New-Object Drawing.Point(20,60)
$buweiInput.Size=New-Object Drawing.Size(600,30)
$buweiButton=New-Object Windows.Forms.Button
$buweiButton.Text='保存本机加密配置'
$buweiButton.Location=New-Object Drawing.Point(20,105)
$buweiButton.Size=New-Object Drawing.Size(180,35)
$buweiStatus=New-Object Windows.Forms.Label
$buweiStatus.Location=New-Object Drawing.Point(20,155)
$buweiStatus.Size=New-Object Drawing.Size(600,50)
$buweiStatus.Text='此窗口不调用模型。回到补位后主动生成建议进行连接验证。'
$buweiButton.Add_Click({
 try {
  if($buweiInput.Text.Length -lt 20 -or $buweiInput.Text.Length -gt 4096 -or $buweiInput.Text -match '[\s\x00-\x1f\x7f]'){throw '请填写完整密钥，不含空白。'}
  $buweiSecretDir=Join-Path $buweiPrivateRoot '.secrets'
  New-Item -ItemType Directory -Path $buweiSecretDir -Force | Out-Null
  $buweiClear=[Text.Encoding]::UTF8.GetBytes($buweiInput.Text)
  try{$buweiSealed=[Security.Cryptography.ProtectedData]::Protect($buweiClear,[Text.Encoding]::UTF8.GetBytes('buwei/minimax-cn/v1'),[Security.Cryptography.DataProtectionScope]::CurrentUser)}finally{[Array]::Clear($buweiClear,0,$buweiClear.Length);$buweiInput.Clear()}
  $buweiPath=Join-Path $buweiSecretDir 'minimax-cn.dpapi'
  $buweiTemporary=$buweiPath+'.'+[Guid]::NewGuid().ToString('N')+'.tmp'
  [IO.File]::WriteAllBytes($buweiTemporary,$buweiSealed)
  if([IO.File]::Exists($buweiPath)){[IO.File]::Replace($buweiTemporary,$buweiPath,$null)}else{[IO.File]::Move($buweiTemporary,$buweiPath)}
  $buweiStatus.Text='已加密保存。返回补位即可主动调用 MiniMax-M3，未进行网络调用。'
 }catch{$buweiStatus.Text='配置未保存。请检查密钥及目录权限，原配置保留。'}
})
$buweiForm.Controls.AddRange(@($buweiLabel,$buweiInput,$buweiButton,$buweiStatus))
try{$null=$buweiForm.ShowDialog()}finally{$buweiInput.Clear();$buweiForm.Dispose()}
