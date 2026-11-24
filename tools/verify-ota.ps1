# 晚上的真机验证: Skyward 自装 OTA + 压缩快照合并
# 用法: 手机进入 recovery 后, 在 PC 上运行:
#   powershell -NoProfile -ExecutionPolicy Bypass -File verify-ota.ps1 -Package <完整包.zip>
# 前提: PC 上已有完整更新包 (10.4 GB 的 zip)

param(
    [Parameter(Mandatory = $true)]
    [string]$Package,
    [switch]$SkipInstall
)

$adb = 'C:\platform-tools\adb.exe'
$fastboot = 'C:\platform-tools\fastboot.exe'
$img = 'D:\文件\work\build\skyward-v0.3.1-beta2-xuanyuan-OS4.0.4.0.XOACNXM.img'
$expect = '7fe901e8e2cb762c2ed3ad19c5cab961ebcadb0ad3fc9ed7d3fb9b0d135a2a71'

function Step($m) { Write-Output ""; Write-Output "=== $m ===" }

Step '0. 前提检查'
if (-not $SkipInstall) {
    if (-not (Test-Path $Package)) { Write-Output "找不到更新包: $Package"; exit 1 }
    Write-Output ("更新包: {0}  {1:N0} 字节" -f $Package, (Get-Item $Package).Length)
}
$hash = (Get-FileHash $img -Algorithm SHA256).Hash.ToLower()
if ($hash -ne $expect) { Write-Output "recovery 镜像哈希不符: $hash"; exit 1 }
Write-Output "recovery 镜像哈希符合: $expect"

Step '1. 手机在 recovery 吗'
& $adb kill-server 2>&1 | Out-Null
& $adb start-server 2>&1 | Out-Null
$state = (& $adb devices | Select-String 'recovery')
if (-not $state) {
    Write-Output '不在 recovery, 自动刷入并进入...'
    & $adb reboot bootloader 2>&1 | Out-Null
    Start-Sleep -Seconds 12
    & $fastboot flash recovery_a $img 2>&1 | Select-String 'Writing|FAILED'
    & $fastboot flash recovery_b $img 2>&1 | Select-String 'Writing|FAILED'
    & $fastboot reboot recovery 2>&1 | Out-Null
    Start-Sleep -Seconds 45
    & $adb kill-server 2>&1 | Out-Null; & $adb start-server 2>&1 | Out-Null
    for ($i = 1; $i -le 8; $i++) {
        if ((& $adb devices | Select-String 'recovery')) { break }
        Start-Sleep -Seconds 10
    }
}
& $adb devices
Write-Output ("slot=" + (((& $adb shell getprop ro.boot.slot_suffix 2>&1) -join '').Trim()))

Step '2. 安装前的状态'
& $adb shell "skyward check 2>&1 | grep -E 'data partition|registered'"
& $adb shell "skyward slot 2>&1"

if (-not $SkipInstall) {
    Step '3. sideload 完整包 (约 10-20 分钟)'
    Write-Output '手机端若弹出确认, 请在屏幕上同意'
    & $adb sideload $Package 2>&1 | Select-Object -Last 6
}

Step '4. 安装后的状态'
& $adb shell "skyward slot 2>&1"
& $adb shell "skyward check 2>&1 | grep 'update in flight'"
& $adb shell "ls /metadata/ota/snapshots 2>/dev/null | tr '\n' ' '; echo"
& $adb shell "cat /metadata/ota/snapuserd_mode 2>/dev/null; echo"
& $adb shell "dmesg | grep -iE 'skyward:.*(mounted|not mounted)' | tail -12"

Step '5. 有压缩快照吗'
$snaps = ((& $adb shell "ls /metadata/ota/snapshots 2>/dev/null" | Measure-Object -Line).Lines)
$mode = ((& $adb shell "cat /metadata/ota/snapuserd_mode 2>/dev/null" 2>&1) -join '').Trim()
Write-Output ("快照数: $snaps  模式: [$mode]")
if (($snaps -gt 0) -and ($mode -eq 'dm-user')) {
    Write-Output ''
    Write-Output '>>> 有真实的压缩快照, 合并命令:'
    Write-Output ('>>>   adb shell skyward merge --compressed')
    Write-Output '>>> 或用 UI: 更新页的合并滑块'
} else {
    Write-Output ''
    Write-Output '>>> 没有压缩快照 (sideload 可能走了 Overwriting 路径)'
    Write-Output '>>> 合并验证要等下次系统内更新'
}

Step '6. 完成后回系统'
Write-Output '确认一切正常后: adb reboot'
