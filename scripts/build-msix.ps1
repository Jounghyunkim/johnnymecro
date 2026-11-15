<#
Builds the Microsoft Store package (MSIX) for Johnny Mecro.

  powershell -ExecutionPolicy Bypass -File scripts\build-msix.ps1              # build app + pack
  powershell -ExecutionPolicy Bypass -File scripts\build-msix.ps1 -SkipBuild   # pack the existing release exe
  powershell -ExecutionPolicy Bypass -File scripts\build-msix.ps1 -SignForTest # also make a self-signed copy for local install

Output: store\JohnnyMecro_<version>_x64.msix  (upload this one to Partner Center; the Store signs it)

Requires the Windows 10/11 SDK (makeappx.exe, makepri.exe). This file is ASCII-only on purpose (Windows PowerShell 5.1).
#>
param(
  [string]$Version = "",
  # Package/Properties/DisplayName: must be a name reserved for this product in Partner Center
  [string]$DisplayName = "johnny-mecro",
  # Name shown in the Start menu / taskbar (uap:VisualElements). Defaults to $DisplayName.
  [string]$AppDisplayName = "",
  [switch]$SkipBuild,
  [switch]$SignForTest
)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$tauri = Join-Path $root "src-tauri"
$outDir = Join-Path $root "store"
$layout = Join-Path $tauri "target\msix\layout"

# ---- version: <tauri version>.0 (the 4th part must be 0 for the Store) ----
if (-not $Version) {
  $conf = Get-Content (Join-Path $tauri "tauri.conf.json") -Raw -Encoding UTF8 | ConvertFrom-Json
  $Version = "$($conf.version).0"
}
if ($Version -notmatch '^\d+\.\d+\.\d+\.0$') { throw "Version must look like 1.2.3.0 (got '$Version')" }
if ($Version -match '^0\.') { throw "Store packages need a major version of 1 or higher (got '$Version')" }

# ---- Windows SDK tools ----
function Find-SdkTool([string]$name) {
  $hit = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter $name -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match '\\x64\\' } | Sort-Object FullName -Descending | Select-Object -First 1
  if (-not $hit) { throw "$name not found. Install the Windows 10/11 SDK." }
  $hit.FullName
}
$makeappx = Find-SdkTool "makeappx.exe"
$makepri = Find-SdkTool "makepri.exe"

# ---- app binary ----
if (-not $SkipBuild) {
  Push-Location $root
  try {
    & npm run tauri build -- --no-bundle
    if ($LASTEXITCODE -ne 0) { throw "tauri build failed" }
  } finally { Pop-Location }
}
$exe = Join-Path $tauri "target\release\johnny-mecro.exe"
if (-not (Test-Path $exe)) { throw "Missing $exe (run without -SkipBuild)" }

# ---- layout ----
if (Test-Path $layout) { Remove-Item $layout -Recurse -Force }
$assets = Join-Path $layout "Assets"
New-Item -ItemType Directory -Force $assets | Out-Null
Copy-Item $exe (Join-Path $layout "johnny-mecro.exe")

# ---- logo assets, rendered from the app icon ----
Add-Type -AssemblyName System.Drawing
$srcPath = Join-Path $tauri "icons\icon.png"
$src = [System.Drawing.Image]::FromFile($srcPath)
function New-Logo([string]$name, [int]$w, [int]$h, [double]$fill) {
  $bmp = New-Object System.Drawing.Bitmap($w, $h, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.Clear([System.Drawing.Color]::Transparent)
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $side = [int][Math]::Round([Math]::Min($w, $h) * $fill)
  $x = [int](($w - $side) / 2)
  $y = [int](($h - $side) / 2)
  $attr = New-Object System.Drawing.Imaging.ImageAttributes
  $attr.SetWrapMode([System.Drawing.Drawing2D.WrapMode]::TileFlipXY)
  $rect = New-Object System.Drawing.Rectangle($x, $y, $side, $side)
  $g.DrawImage($src, $rect, 0, 0, $src.Width, $src.Height, [System.Drawing.GraphicsUnit]::Pixel, $attr)
  $g.Dispose()
  $bmp.Save((Join-Path $assets $name), [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
}
foreach ($scale in 100, 200, 400) {
  $k = $scale / 100
  New-Logo "StoreLogo.scale-$scale.png" (50 * $k) (50 * $k) 1.0
  New-Logo "Square44x44Logo.scale-$scale.png" (44 * $k) (44 * $k) 1.0
  New-Logo "SmallTile.scale-$scale.png" (71 * $k) (71 * $k) 0.8
  New-Logo "Square150x150Logo.scale-$scale.png" (150 * $k) (150 * $k) 0.62
  New-Logo "Wide310x150Logo.scale-$scale.png" (310 * $k) (150 * $k) 0.62
}
foreach ($scale in 100, 200) {
  $k = $scale / 100
  New-Logo "LargeTile.scale-$scale.png" (310 * $k) (310 * $k) 0.55
}
# taskbar / Start list / Alt+Tab icons (unplated = no accent-colour plate behind the icon)
foreach ($size in 16, 24, 32, 48, 256) {
  New-Logo "Square44x44Logo.targetsize-$size.png" $size $size 1.0
  New-Logo "Square44x44Logo.targetsize-${size}_altform-unplated.png" $size $size 1.0
  New-Logo "Square44x44Logo.targetsize-${size}_altform-lightunplated.png" $size $size 1.0
}
$src.Dispose()

# ---- manifest ----
$manifest = Get-Content (Join-Path $tauri "msix\AppxManifest.xml") -Raw -Encoding UTF8
if (-not $AppDisplayName) { $AppDisplayName = $DisplayName }
$manifest = $manifest.Replace("{{VERSION}}", $Version).Replace("{{DISPLAY_NAME}}", [System.Security.SecurityElement]::Escape($DisplayName)).Replace("{{APP_DISPLAY_NAME}}", [System.Security.SecurityElement]::Escape($AppDisplayName))
[IO.File]::WriteAllText((Join-Path $layout "AppxManifest.xml"), $manifest, (New-Object System.Text.UTF8Encoding($false)))

# ---- resources.pri (maps Assets\X.png to its scale-/targetsize- variants) ----
$priCfg = Join-Path $tauri "target\msix\priconfig.xml"
& $makepri createconfig /cf $priCfg /dq ko-KR /pv 10.0.0 /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makepri createconfig failed" }
# one resources.pri for the whole package (no per-scale / per-language resource packs)
$cfg = Get-Content $priCfg -Raw
$cfg = [regex]::Replace($cfg, '(?s)<packaging>.*?</packaging>', '')
[IO.File]::WriteAllText($priCfg, $cfg)
& $makepri new /pr $layout /cf $priCfg /mn (Join-Path $layout "AppxManifest.xml") /of (Join-Path $layout "resources.pri") /o | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makepri new failed" }

# ---- pack ----
New-Item -ItemType Directory -Force $outDir | Out-Null
$msix = Join-Path $outDir "JohnnyMecro_${Version}_x64.msix"
& $makeappx pack /d $layout /p $msix /o
if ($LASTEXITCODE -ne 0) { throw "makeappx pack failed" }
Write-Host ""
Write-Host "Store package: $msix"

# ---- optional: self-signed copy for a local install test (NOT for upload) ----
if ($SignForTest) {
  $signtool = Find-SdkTool "signtool.exe"
  $subject = ([xml]$manifest).Package.Identity.Publisher
  $testDir = Join-Path $tauri "target\msix\test"
  New-Item -ItemType Directory -Force $testDir | Out-Null
  $cert = New-SelfSignedCertificate -Type Custom -Subject $subject -KeyUsage DigitalSignature -FriendlyName "Johnny Mecro MSIX test" `
    -CertStoreLocation "Cert:\CurrentUser\My" -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
  $cer = Join-Path $testDir "JohnnyMecro_test.cer"
  Export-Certificate -Cert $cert -FilePath $cer | Out-Null
  $testMsix = Join-Path $testDir "JohnnyMecro_${Version}_x64_test.msix"
  Copy-Item $msix $testMsix -Force
  & $signtool sign /fd SHA256 /sha1 $cert.Thumbprint $testMsix
  if ($LASTEXITCODE -ne 0) { throw "signtool failed" }
  Write-Host ""
  Write-Host "Test package:  $testMsix"
  Write-Host "To install it locally (administrator PowerShell):"
  Write-Host "  Import-Certificate -FilePath `"$cer`" -CertStoreLocation Cert:\LocalMachine\TrustedPeople"
  Write-Host "  Add-AppxPackage `"$testMsix`""
  Write-Host "Afterwards: Get-AppxPackage JohnnyStudio.johnny-mecro | Remove-AppxPackage, and delete the certificate from"
  Write-Host "  Cert:\LocalMachine\TrustedPeople and Cert:\CurrentUser\My (thumbprint $($cert.Thumbprint))."
}
