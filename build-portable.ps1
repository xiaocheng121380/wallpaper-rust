# WallCraft Portable Build Script
Write-Host '=====================================' -ForegroundColor Cyan
Write-Host '  WallCraft Portable Build' -ForegroundColor Cyan
Write-Host '=====================================' -ForegroundColor Cyan
Write-Host ''

$exePath = 'src-tauri\target\release\wallcraft.exe'
if (-not (Test-Path $exePath)) {
    Write-Host 'Error: Build not found' -ForegroundColor Red
    Write-Host 'Please run: pnpm run tauri:build' -ForegroundColor Yellow
    exit 1
}

Write-Host '[1/6] Check build... OK' -ForegroundColor Green

$portableDir = 'portable'
if (Test-Path $portableDir) {
    Remove-Item -Path $portableDir -Recurse -Force
}
New-Item -ItemType Directory -Path $portableDir -Force | Out-Null
Write-Host '[2/6] Create directory... OK' -ForegroundColor Green

Copy-Item -Path $exePath -Destination "$portableDir\WallCraft.exe"
Write-Host '[3/6] Copy executable... OK' -ForegroundColor Green

$binDir = 'src-tauri\bin'
if (Test-Path $binDir) {
    New-Item -ItemType Directory -Path "$portableDir\bin" -Force | Out-Null
    # Copy all FFmpeg tools (ffmpeg, ffplay, ffprobe)
    $essentialFiles = @('ffmpeg.exe', 'ffplay.exe', 'ffprobe.exe')
    $copiedCount = 0
    foreach ($file in $essentialFiles) {
        $srcFile = Join-Path $binDir $file
        if (Test-Path $srcFile) {
            Copy-Item -Path $srcFile -Destination "$portableDir\bin\" -Force
            $copiedCount++
        }
    }
    Write-Host "    - Copied $copiedCount files (ffmpeg, ffplay, ffprobe)" -ForegroundColor Gray
} else {
    Write-Host '    Warning: bin directory not found' -ForegroundColor Yellow
}
Write-Host '[4/6] Copy dependencies... OK' -ForegroundColor Green

$wallpapersDir = 'data\wallpapers'
if (Test-Path $wallpapersDir) {
    Copy-Item -Path $wallpapersDir -Destination "$portableDir\data\wallpapers" -Recurse -Force
    Write-Host '[5/6] Copy wallpapers... OK' -ForegroundColor Green
} else {
    Write-Host '[5/6] wallpapers not found' -ForegroundColor Yellow
}

'WallCraft Portable v0.1.0' | Out-File -FilePath "$portableDir\README.txt" -Encoding UTF8
Write-Host '[6/6] Create README... OK' -ForegroundColor Green

Write-Host ''
Write-Host 'Creating ZIP...' -ForegroundColor Yellow
$zipName = 'WallCraft-0.1.0-Portable.zip'
if (Test-Path $zipName) { Remove-Item $zipName -Force }
Compress-Archive -Path "$portableDir\*" -DestinationPath $zipName -Force

Write-Host ''
Write-Host 'Build Complete!' -ForegroundColor Green
Write-Host "ZIP: $zipName" -ForegroundColor Cyan
