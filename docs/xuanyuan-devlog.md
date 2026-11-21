# xuanyuan 适配开发记录

小米 15 Ultra（`xuanyuan`）、澎湃 OS 4.0.4.0（`OS4.0.4.0.XOACNXM`）的 Skyward 适配。
按时间倒序，记录做过什么、为什么这么做、验证到哪一步、还欠什么。

---

## 2026-11-22（夜）—— 解密崩溃的根因与回退

**现象**：`/data` 解密在点「解密」后黑屏重启，分区仍未挂载。

**根因**（内核日志，`recovery.log` 每次启动会清空，必须看 `dmesg`）：

```
249.504  servicemanager: Caller(pid=829, uid=1000, sid=u:r:recovery:s0)
249.893  skyward: ending key service pid 829 so it reads the installed system's version again
249.894  smcinvoke: server_release: android.hardwar server terminated
         ← 日志到此为止，下一行是 255 秒的重启
```

`pin()` 用 `running(KEYMINT_PROGRAM)` 在 `/proc` 里找 keymint 的 pid，**找到的是 recovery
自己的 pid**，然后 `kill(pid, SIGTERM)` 把自己杀了。

**处置**：

1. 两道防线：`running()` 跳过自身 pid；`pin()` 里 `ensure!(pid != me)`
2. 回退 `6034b59`（另一条支线的 `secure.rs`：把硬件驱动创建挪到 loader 之前）——它改完之后
   才出现崩溃，是首要嫌疑
3. **实测：解密恢复正常**

**顺带发现的一个流程 bug（值得记）**：用 `tar` 从 Windows 同步源码会**保留原 mtime**，
cargo 看到源文件比产物旧就跳过编译（`Finished in 0.10s`），于是**镜像是旧的**。曾因此连续
两次把旧二进制刷进去而不自知。已在 `wsl-build.sh` 里加 `find ... -exec touch` 修掉。

---

## 2026-11-22 —— 浏览器：CSS、图片、性能

**做了什么**：

| 能力 | 说明 |
| --- | --- |
| CSS | 新模块 `skyward-ui/src/style.rs`。只读影响"文字怎么站"的声明：`display`（含 `none`）、`font-size`（px/em/rem/%）、`color`/`background`、`text-align`、`font-weight`、上下间距。不做级联（最后一条规则胜）、不读 `@media`、没有布局算法 |
| 样式继承 | 每个块带它所在元素栈上累加出来的样式 |
| 图片 | 新模块 `skyward-ui/src/image.rs`。PNG/JPEG/WebP，按字节头判断格式（不信 `Content-Type`）。单张上限 2MB、1600 万像素。随页面一次取最多 4 张 |
| 绘制 | 按行直写 canvas 字节，不透明像素直接拷、透明才混合 |
| 视频 | 识别 `<video>`/`<audio>`/`<source>`，显示成卡片并声明本浏览器不能播放 |
| 链接 | 相邻链接之间补空格（B站是 `</a><a>` 连着写）；连续 3 个以上「单链接短块」合并成一行 |
| 需要 JS 的页面 | `script ≥ 5 且正文块 ≤ 2` 时显示明确提示，而不是白屏 |

**性能**（这是"浏览器巨卡"的来源，已修）：

- 图片原本逐像素走 `blend`（一张照片 54 万次函数调用 + 边界检查）→ 改按行直写
- 长页画布 3 屏（55MB）→ 2 屏（37MB）
- 一页取图 8 张 → 4 张，单图最高 420dp → 320dp

**结论（重要，写给后面的人）**：要让云·原神这类站点能跑，需要 **JS 引擎 + DOM + CSSOM +
WebRTC/HLS + 音频 + 输入转发**，那是整个浏览器的量级。实测 `ys.mihoyo.com/cloud/` 只有
5833 字节、15 个 `<script>`，去掉标签后可见文字只有「云·原神」四个字——纯 JS 应用。
**已确认不可行**，浏览器因此暂时下线（见下）。

---

## 2026-11-22 —— 侧滑返回、电池、上游对齐

**侧滑返回没有提示箭头**：根因是本地提交 `f8a4d7c`（做「页面跟手预览」时）在绘制处加了
`if self.preview.is_none()`，把整个返回提示挡掉了；**上游从来没有这行**。已逐行恢复上游写法
（波浪 + `<`，无论有没有预览都画）。

**充电时不显示闪电**：`Battery::charging()` 只看电量计报的 `status`，没看充电器是否在线
（`plugged` 字段一直读了但没用）。在 recovery 里手机常插着电而电量计仍报 `Discharging`，
因为平时启动充电的是 Android 框架。已改为 `self.plugged || matches!(charge, Charging | Full)`。

**顺手修的健壮性**：电量计报出不认识的词（如 `Charging fast`）时，原来会让整个电池读数
失败（电量也丢了），现在记为 `Unknown` 但保留电量。

---

## 2026-11-21 —— 视频解码器：可以加载出来

在真机上验证：

```bash
insmod /vendor/lib/modules/msm_video.ko     # 成功
```

结果：`/dev/video32`（解码器）、`/dev/video33`（编码器）出现，
固件 `vpu35_4v.mbn` 从 `/odm/firmware/` 加载成功（10ms）。

```
msm_vidc_v4l2 aa00000.qcom,vidc: bound ... (ops msm_vidc_component_ops [msm_video])
```

**结论**：这台手机的视频硬件解码器在 recovery 里能用，只是模块默认没加载。
直链视频播放（不含 DRM、不含 JS 取流）技术上可行，尚未实现。

---

## 更早：核心适配（已完成并验证）

- **`/data` 解密**：metadata 加密 + `wrappedkey_v0`，keymint/gatekeeper/weaver/eSE 全链路；
  实测 `/data/data` 607 个目录、`/sdcard` 可读
- **`-62 KEY_REQUIRES_UPGRADE`**：必须调 `upgradeKey`（用续期后的 blob），只重启服务无效
- **压缩快照（VABC）**：把手机自己的 `snapuserd` 打进 ramdisk；内核 target 名是 `user`
  不是 `dm-user`；表参数必须 3 段 `0 0 <名字>`；`init` 要 5 段
- **屏幕/触摸**：1440×3200@120、双击唤醒
- **OTA 警告**：在 recovery 里装官方完整包大概率卡米（自 OS4 起 bootloader 有自己的签名校验，
  `disable-verification` 绕不过）。安装页与系统更新页都有红字

---

## 待办

- [ ] **安装/更新日志**：像 TWRP 一样实时滚动、能滑到底部查看（用户标为最高优先）
- [ ] **脚本里的音量键**：部分官改包的 `update-binary` 用
      `getevent -qlc 1 | grep VOLUME` 读音量键。Skyward 没有 `EVIOCGRAB`，理论上不冲突，
      但要确认 ramdisk 里有 `getevent`，且脚本运行期间 UI 不吞掉按键
- [ ] 全部参数与信息页底部有黑块
- [ ] 浏览器：暂时下线，后续再适配
- [ ] 直链视频播放（V4L2 硬件解码）
- [ ] 云·原神这类 SPA：不可行，除非实现完整 JS+DOM
