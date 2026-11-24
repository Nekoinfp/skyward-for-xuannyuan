# Skyward for Xiaomi 15 Ultra (xuanyuan)

Skyward 是一个用 Rust 写的第三方 recovery。

本项目是 **小米 15 Ultra（xuanyuan）、澎湃 OS 4.0.4.0（OS4.0.4.0.XOACNXM）** 的适配版本。

- 上游：[kirklin/skyward](https://github.com/kirklin/skyward) 0.3.1
- 本版本：**0.3.1-beta2**
- 目标机型：Xiaomi 15 Ultra（`25019PNF3C` / `25010PN30C`）
- 测试系统：OS4.0.4.0.XOACNXM（Android 17 / SDK 37 + vendor SDK 35）

> ⚠️ **测试版**。已经在一台真机上逐项验证过下面列出的功能，
> 刷入前请自己备份原厂 recovery。

---

## 已验证的功能

在真实设备上逐项跑过：

- **`/data` 完整解密**：输入锁屏 PIN 后，设备加密（DE）与用户加密（CE）都打开——
  `/data/data` 可读、`/sdcard` 可读。这是本适配修好的核心问题。
- **压缩快照读取**：系统更新后产生的 9 个 lz4 用户态快照，全部挂载成功，
  六个密钥服务从快照里的分区正常注册。
- **界面、屏幕、触摸、WLAN**：都可用。
- **解密后查看文件、备份/恢复、终端、挂载管理**等页面功能。
- **安装**：`adb sideload`、内部存储、U 盘三种来源；可一次选多个 zip/img 依次安装；
  刷单个 `.img` 时可选择只刷指定分区（boot / init_boot / vendor_boot / recovery /
  dtbo / vbmeta）与槽位。
- **逻辑分区扩容规划**：`skyward lp --plan-grow` 只读地算一遍能不能扩（见下面"已知限制"）。

## 已知限制

- **不要在 recovery 里装官方完整包**。在 Skyward（或任何第三方 recovery）里刷官方
  完整 OTA，装完后大概率卡米：安装本身是成功的，但更新包会把 boot 与 vbmeta 刷成
  原厂，而自 os4 后小米的 bootloader 有自己的签名校验，`disable-verification` 标志
  绕不过去。装完两个槽都会变成"不可启动"，连 recovery 都进不去，只剩 fastboot。
  **请始终在系统里更新**（设置 → 系统更新）。安装页与系统更新页都已用红字标明这条。
  （如果变砖保数据，请线刷官方完整包）
- **压缩快照的合并**没有开启。`skyward merge --compressed` 会逐分区驱动手机的
  snapuserd 合并，但实测 9 个分区里只有 1 个（COW 在 super 里那个）能完成，其余停在
  `snapshot-merge`。退路是可靠的：重启进系统，系统会在一分钟内自己合并完。
- **逻辑分区就地扩容做不到**。这台手机的 super 元数据里没有对齐的空闲区（唯一空洞
  1 MB），要扩必须搬 extent，而 `grow` 按设计不移动别人的扇区。OTA 不受影响。
- **增量更新包与非 A/B 更新包未实现**（前者等上游发布新版本可测，后者这台机器用不上）。

---

## 安装

手机进入 fastboot 刷入镜像即可。

```bash
fastboot flash recovery_a skyward-v0.3.1-beta2-xuanyuan-OS4.0.4.0.XOACNXM.img
fastboot flash recovery_b skyward-v0.3.1-beta2-xuanyuan-OS4.0.4.0.XOACNXM.img
fastboot reboot recovery
```

下载见 [Releases](https://github.com/Nekoinfp/skyward-for-xuannyuan/releases)。

校验（可选）：

```bash
sha256sum skyward-v0.3.1-beta2-xuanyuan-OS4.0.4.0.XOACNXM.img
# 84bf36e7d2431034035948faf5d0243f9ce150d596a2d3dbd7f670f8ae22000c
```

---

## 许可与致谢

继承上游 Skyward：**GPL-3.0-or-later**，详见 [LICENSE](LICENSE)。

上游仓库：[kirklin/skyward](https://github.com/kirklin/skyward) —— 绝大多数设计与代码来自这里。

界面字体 MiSans（小米，SIL OFL 1.1），终端等宽字体 Kirk Mono（SIL OFL 1.1）。
