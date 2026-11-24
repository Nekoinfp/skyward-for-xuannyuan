# xuanyuan 适配验证报告

这份报告汇总的是**在本机（Linux/WSL，无手机）上能够确证的一切**，以及**真机上还需要验证什么**。
一句话结论：

> 解密链、逻辑分区扩容、sparse super 写入这三块的实现与前提都已核对到实物固件层面，
> 并有可复现的测试；但**镜像没有在任何手机上运行过**，压缩快照/增量 OTA 未实现。

凡是本节没有写"实测"的，都不是实测。

---

## 一、交付物

| 项 | 值 |
| --- | --- |
| 镜像 | `skyward-v0.3.1-beta2-xuanyuan-OS4.0.4.0.XOACNXM.img` |
| 大小 | 104,857,600 字节（与底包 recovery 分区同尺寸） |
| SHA-256 | `84bf36e7d2431034035948faf5d0243f9ce150d596a2d3dbd7f670f8ae22000c` |
| 形状 | `ANDROID!`、header_version 4、kernel 0、ramdisk 51,988,960 字节 |
| 内含 recovery 本体 | 9,047,864 字节，AArch64 ELF（另含 `system/bin/snapuserd` 1,273,320 字节，读压缩快照用） |
| 底包 | OS4.0.4.0.XOACNXM，recovery 分区 SHA-256 `55f491052a360bc15362fd80c8bd2dd0912b289e1653f2bd7f9c5a12e7fea27b`（打包器自行核对） |

结构审计（`tools/audit-image.py`）32 项全过：boot 头、ramdisk 解包为 newc 并以
`TRAILER!!!` 结束、`system/bin/recovery` 为 AArch64 ELF、`system/bin/skyward` 指向
`recovery`、AVB footer 指向的 vbmeta 中哈希描述符的摘要**确实等于
`sha256(salt + 镜像)`**、属性描述符携带底包的 fingerprint。

同一份镜像重建两次哈希相同（构建可复现）。

---

## 二、可复现的验证

在 Linux（WSL 里的 Ubuntu 24.04，rustc 1.100.0，NDK r29）上执行 CI 自己的命令：

| 命令 | 结果 |
| --- | --- |
| `cargo test --workspace --locked` | 45 个 `test result` 块全绿，0 失败 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 退出码 0 |
| `cargo ndk -t arm64-v8a --platform 30 build --locked --release -p skyward` | 退出码 0 |
| 两个静态首程序（`RUSTFLAGS=-C target-feature=+crt-static`） | 退出码 0 |
| `cargo build --locked --release -p skyward-pack` | 退出码 0 |

其中与本适配直接相关的用例数：`skyward-install` 169、`skyward-lp` 23、
`skyward-block` 37、`skyward-pack` 13、`skyward-modules` 16、`skyward` 二进制自己 26（报告 6、硬件描述与实例冲突 7、库拷贝 3、目录搜索 6、`art` 缓存标记 4），`skyward-sys` 47（含描述符移交与"进程还在跑吗"）、`skyward-vab` 39。

稳定性：整个 workspace 连跑 15 次无失败；修掉一处偶发（并行测试把彼此的 `flock`
当成对手）后，avbctl 连跑 60 次无失败。

顺带修好了这份 fork **原本就存在**的三处失败（`copy()` 读出错时丢缓冲字节、
`spans()` 把正好三窗口当"太短"、`unzip -t` 从不报坏 CRC）——它们原本会让 CI 一直是红的。

---

## 三、对固件的实物核对

把 4.0.4.0 底包里的分区逐个取出、挂载、核对（全部在本机完成）：

| 分区 | 大小 | 核对到的结论 |
| --- | --- | --- |
| `recovery` | 104,857,600 | 哈希与配置一致；72 个 `system/lib64/*.so` |
| `vendor_dlkm` | 87,343,104 | 391 个模块；配置点名的 4 个都在；`modules.dep` 在 |
| `system_dlkm` | 15,282,176 | 有内容的是 `flatten/lib/modules`（96 个模块 + `modules.dep`），`lib/modules` 本身是空的 |
| `vendor` | 742,723,584 | `/vendor/etc/fstab.qcom` 的 `/data` 行含 `metadata_encryption=aes-256-xts:wrappedkey_v0`；配置启动的 9 个程序全在；`libhardware.so`、`lib64/hw` 在；`libvndksupport.so` **不在** |
| `odm` | 4,563,202,048 | weaver 服务在；`ese_weaver_thales.so`、`mi_weaver.so` 在 `/odm/lib64`；`libjc_keymint*.so` 属于 StrongBox 那个 keymint，不是本配置启动的 |
| `system` | 977,719,296 | 925 个库（recovery 只有 72）；`libvndksupport.so` 在 `system/lib64`，`libdl_android.so` 在 `system/lib64` 是链接进 runtime APEX、真副本在 `bootstrap/`；`system/bin/snapuserd` 与 `libsnapshot.so` 都在 |

三条关键结论：

1. **`/data` 是包裹式加密**：元数据密钥由 `wrappedkey_v0` 包着，解包要走 keymint，
   锁屏保护走 weaver。所以"密钥服务起不来 → /data 打不开"的因果成立。
2. **缺的库只有两个，位置还不同目录**：
   `keymint → libhardware.so(vendor) → libvndksupport.so(仅 system) →
   libdl_android.so(system/bootstrap)`。`provide_system_libraries()` 逐个库搜索
   `lib64` 与 `lib64/bootstrap`，两条都取得到；只查 `lib64` 会漏第二个。
3. **整张 fstab 解析无误**：62 行非注释条目 → 62 条解析结果，`/data` 两行
   （f2fs 与 mifs）字段全对。真机那两行已作为回归用例。

**自己复现这套核对**——一条命令（需要 cargo；`--firmware` 那步需要 root 或能 loop 挂载）：

```
skyward-pack stock --config devices/xuanyuan.toml --out stock/recovery.img --partition vendor
skyward-pack stock --config devices/xuanyuan.toml --out stock/recovery.img --partition odm
sudo mount -o loop,ro stock/vendor.img /mnt/vendor
sudo mount -o loop,ro stock/odm.img /mnt/odm

tools/verify.sh --config devices/xuanyuan.toml \
    --image out/skyward-v0.3.1-beta2-xuanyuan-OS4.0.4.0.XOACNXM.img \
    --firmware /mnt
```

（`--firmware` 指的那个目录下要有 `vendor/` 与 `odm/` 两个子目录。）

输出（实测）：

```
cargo test --workspace --locked                ok
cargo clippy --workspace --all-targets -- -D warnings ok
the stock recovery's SHA-256 is the one named  ok
the image audits (boot header, ramdisk, AVB digest) ok
every path the configuration names is in the firmware ok

5 passed, 0 failed, 0 skipped
```

脚本的取舍：分区没挂上时它**说"分区没挂上"并跳过**，而不是把每条路径都报成缺失；
任何一项失败都以非零退出，可以直接放在 CI 或发布脚本里。

`paths` 只核 `/vendor` 与 `/odm`：`/system` 下的路径是打包时塞进 ramdisk 的
（`se_omapi`、recovery 自己的 `lib64`），不在手机的这两个分区里，命令会把它们单列出来。
某个分区没给全时命令**直接失败**并说明该加哪个 `--`，不让"只查了一半"冒充通过。

### 真机实测（2026-11-19，首次）

一台已解锁**未**成功的小米 15 Ultra 经 USB 调试连上电脑，只读地取了以下数据。

**设备身份与状态**

| 项 | 值 |
| --- | --- |
| 机型 | `xuanyuan` / 25019PNF3C |
| 系统 | `OS4.0.4.0.XOACNXM`（与配置里的底包一致） |
| Android | **17（SDK 37）**；`ro.vendor.build.version.sdk` = **35** |
| 安全补丁 | `2026-10-01`（与从底包提取的值一致） |
| 当前槽 | `_a` |
| bootloader | **已锁**：`verifiedbootstate=green`、`device_state=locked`、`devicelock=1` |
| OEM 解锁开关 | **`oem_unlock_allowed=0`（关闭）** |
| 区域 | CN |

**与固件核对一致的项**

| 项 | 结果 |
| --- | --- |
| `/vendor/etc/fstab.qcom` 第 88 行 | 与我从底包提取的**逐字节相同**：`metadata_encryption=aes-256-xts:wrappedkey_v0`、`fileencryption=…wrappedkey_v0`、`keydirectory=/metadata/vold/metadata_encryption` |
| `/odm/etc/fstab.qcom` | 这台**没有**（代码按 NotFound 跳过） |
| `libvndksupport.so` | 51,504 字节，真机与镜像**同大小** |
| `/dev/dm-user/` | 存在（当前为空） |

**新得到、且改变判断的三条**

1. **`ro.virtual_ab.userspace.snapshots.enabled = true`**，`compression.enabled = true`。
   也就是说**压缩快照（VABC）是这台机器正常的 OTA 通道**。于是"未合并的压缩快照会让
   `mount_partitions()` 整体失败、`/data` 解不开"不是假设，是**真实可达的状态**。
2. **版本落差是真的**：系统 SDK 37 / Android 17，而 vendor 是 SDK 35、bootloader 的
   `swinfo.fingerprint` 写着 `xuanyuan:15/…`。这验证了打包器剔掉版本属性、
   由 recovery 从已安装系统 adopt 的必要性——差的是**两个大版本**，不是小补丁。
3. **一处纠错**：`/system/lib64/libdl_android.so` 在真机上是**指向 runtime APEX 的符号链接**，
   真实副本在 `bootstrap/`。我先前只看镜像时记成"只在 bootstrap"，是 `[ -e ]` 跟随悬空
   链接导致的误判。见第三节的更正。

**结论：这台机器刷了，而且 `/data` 解开了。** 起先 `ro.boot.verifiedbootstate=green`、
`ro.boot.flash.locked=1`、`oem_unlock_allowed=0` 读起来像未解锁，实际是装在机上的
Magisk 模块（`tricky_store` 等）把状态伪装了。`fastboot getvar unlocked` 给出
**`unlocked: yes`**——fastboot 直连 bootloader，模块改不了这个值。

**刷入与结果（2026-11-19）**

```
fastboot flash recovery_a skyward-v0.3.1-beta2-xuanyuan-OS4.0.4.0.XOACNXM.img   OKAY
fastboot flash recovery_b skyward-v0.3.1-beta2-xuanyuan-OS4.0.4.0.XOACNXM.img   OKAY
fastboot reboot recovery
```

第一次启动后 `skyward check` 报出**六个密钥服务全部 NOT registered**。查 dmesg 拿到根因：

```
servicemanager: getRecoveryHalManifest: VINTF parse error: Cannot add manifest fragment
/system/etc/vintf/manifest/weaver-thales.xml: HAL "android.hardware.weaver" has a conflict:
Conflicting FqInstance: IWeaver/default (@2) (from android.hardware.weaver-service.thales.xml)
vs. IWeaver/default (@1) (from weaver-thales.xml).
```

两份描述声明了同一个实例：ramdisk 里打包器放的 `weaver-thales.xml`（@1）与已安装系统提供的
`android.hardware.weaver-service.thales.xml`（@2）。**读者遇到冲突会丢掉整份清单**，
于是一个服务都找不到——gatekeeper 报 "Could not find … in the VINTF manifest"，
secure_element 连懒启动都失败。

修法是让**已安装系统的描述优先**：写入系统的描述时，删掉同目录下声明了同一实例的旧描述。
修好后重新刷入，`skyward check` 变成：

```
the services the key chain needs
  registered: android.hardware.security.keymint.IKeyMintDevice/default
  registered: android.hardware.gatekeeper.IGatekeeper/default
  registered: android.hardware.secure_element.ISecureElement/eSE1
  registered: android.se.omapi.ISecureElementService/default
  registered: android.hardware.weaver.IWeaver/default
  registered: android.hardware.boot.IBootControl/default
```

**`/data` 解开了：**

```
/dev/block/dm-9 on /data type f2fs (rw,...,inlinecrypt,...)
477G  366G 已用  111G 可用        /data
```

`/data/media/0`、`/data/system/users/0` 都是真实的用户数据内容。

这一条是本适配从第一天起要解决的事，现在**在真机上成立**。

**但要说明白它成立的方式**：服务注册好之后，`/data` 还需要**用户凭据**才挂得上。
第二次进 recovery 时 `skyward check` 里六个服务同样全部 `registered`、密钥文件也读得到，
可 `/data` 没有挂——recovery 自己的日志写着：

```
remembering the decrypt password is on, and no credential is kept
```

也就是它**停在解密页等输入 PIN**。第一次能读到 `/data`，是因为当时有人在屏幕上输了密码。
这是设计行为，不是缺陷：FBE 的元数据密钥要用用户凭据解包。
（凭据没有任何一条被记录，`skyward check` 也只列密钥文件名和大小、不读内容。）

### 真机第二轮：直接问 HAL，以及界面

前两轮只证明"服务注册了"。这一轮**直接向三个 HAL 提问**，答案比注册更有分量。

**KeyMint 说出了它被告诉的版本——这是整个适配最关键的一条**

```
keymint Keymint HAL: 3 version 3 at security level 1
  tag 805307073 (OS_VERSION)        = 170000      ← Android 17
  tag 805307074 (OS_PATCHLEVEL)     = 202610      ← 2026-10
  tag 805307086 (VENDOR_PATCHLEVEL) = 20261001
```

安全世界被告知的是**已安装系统的 Android 17 / 补丁 2026-10**，不是 recovery 自己那套
过时的 15。打包器剔掉版本属性、由 `adopt_installed_version` 写入、`environment::pin()`
重启 keymint 这一整套设计，在真机上成立。密钥被拒的那类失败（补丁级别被读旧）
因此不会发生。

**Weaver 与 Gatekeeper 也都应答**

```
skyward weaver     → the weaver has 128 slots, keeps keys of 16 bytes and values of 16 bytes
skyward gatekeeper → the credential service answers for android.hardware.gatekeeper.IGatekeeper
                     it serves version 1 of that interface
                     问一个不属于任何人的凭据：the secure world does not recognise that credential
```

gatekeeper 对"不存在的凭据"给出正确的拒绝，说明它不只是注册了，而是**在处理请求**。

**界面：用 `skyward capture` 取画面，再与离线预览比对**

recovery 里没有 `fb0`，但 `skyward capture` 从 `/dev/dri/card0` 读出整个画面
（1440×3200 RGBA，18,432,000 字节）。把它和 `skyward-preview` 渲染的 244 张页面逐一比对：

| 页面 | 平均差异（/255） |
| --- | --- |
| **112-home-keys-dark** | **6.66** |
| 187-home-title-going-dark | 10.56 |
| 其余 | ≥ 10.85 |

**真机画面就是深色主页，与离线渲染同一页的差异只有 6.66/255**——渲染管线
（DRM/KMS、字体、布局）与设计一致。

**更正：`skyward data` 并没有崩溃，之前的 "Aborted" 是我自己造成的**

早先记录说它中止了。再跑一次、这回把输出写进文件而不是接 `| head`，结果是**成功**：

```
Info: checkpoint state = 42 :  crc orphan_inodes sudden-power-off
Info: No error was reported
the check of /dev/block/dm-9 ended with 0
/dev/block/dm-9 holds f2fs at /data
user 0's own files stay closed
user 999's own files stay closed
```

那时我写的是 `skyward data 2>&1 | head -14`：`head` 读够 14 行就把管道关掉，
进程写下一行时收到 SIGPIPE 而死，shell 就报 "Aborted"。**是观察方式的问题，不是程序的问题。**

这次也顺带确认了它真正做的事：它调用的是 `fsck.f2fs -a`（自动修复模式）——
这正是 Android 自己在挂载带 `check` 标志的文件系统时做的；然后**用元数据密钥把
`/data` 挂起来，不需要用户凭据**，并如实说明用户自己的文件（CE）仍然关着。

也就是说：**`/data` 的设备加密部分（DE）不用 PIN 就能读**
（`/data`、`/data/media/0` 都可读），**用户 CE 存储要等凭据**。这两件事现在分开说清楚了。

**`skyward check` 现在会直说数据分区开没开**（这一轮补上的）：

```
  the data partition: open, its files can be read
  the data partition: closed, so nothing in it can be read yet
  the users' own files: 0 closed, 999 closed
```

真机上第二次启动那种"服务全注册、但 `/data` 没挂"的状态，现在一眼就能看出来。

### 真机第三轮：只读命令与硬件

修好之后又在真机上跑了几个**只读**子命令，结果都落在预期上。

**`skyward lp` —— 解析器面对小米真实的 super 元数据**

```
slot 0: version 10.2, flags 0x1
geometry: 3 metadata slots of 65536 bytes, logical block 4096 bytes
device super: first sector 2048, 13421772800 bytes, alignment 4096 offset 0
group qti_dynamic_partitions_a: max 13411287040 bytes
partition odm_a … 4563202048 bytes      partition system_a … 977719296 bytes
partition product_a … 5273763840 bytes  partition vendor_a … 742723584 bytes
partition vendor_dlkm_a … 87343104      partition system_dlkm_a … 15282176
partition system_ext_a … 817664000      partition mi_ext_a … 170807296
partition mi_product_a … 348160         partition odm_a-cow group cow … 771870720
```

**每个分区大小都与我从底包里提取的镜像逐字节一致**（odm 4,563,202,048、system
977,719,296、vendor 742,723,584、vendor_dlkm 87,343,104、system_dlkm 15,282,176）。
`--slot 1` 同样解析正常。这是 LP 解析器第一次面对真实元数据——此前只用合成数据测过。

有两点只有真机才看得到：每个分区是**多个 extent 拼接**的（odm_a 有 10 段），
而 `odm_a-cow` 已经在 `cow` 组里、占 771,870,720 字节。

**`skyward info` —— 读数与配置对得上**

```
model = Xiaomi 15 Ultra (xuanyuan)     cpu = 8 cores, max 4320000 kHz
ram = 15467980 KiB                     panel = (1440, 3200)   ← 与配置一致
os_version = OS4.0.4.0.XOACNXM         android = 17
patch = 2026-10-01                     kernel = 6.6.118-android15-…
storage = 512403419136 bytes           battery = 49%, 28.4°C, cycles 522
boot = Boot { unlocked: Some(true), verified: Some("orange") }
partitions = 141 under /dev/block/by-name
```

`verified: "orange"` 是**确证**：在系统里读到的 `green` 是 Magisk 模块伪装的结果。

**屏幕、触摸、WLAN**

| 项 | 结果 |
| --- | --- |
| 显示 | `skyward: the display is DSI connector 67 mode "1440x3200x120cmd" 1440x3200 120 Hz` —— Skyward 自己把屏点起来了 |
| 背光 | `panel0-backlight/brightness = 546`（非零） |
| 输入 | `/dev/input/event0..7` 都在，`synaptics_tcm2`、`xiaomi_touch` 已加载 |
| eSE 模块 | `stm_nfc_i2c`、`stm_st54se_gpio` 已加载 |
| WLAN | `wlan0`/`p2p0`/`wifi-aware0` 都在，`wpa_supplicant` 在跑（pid 798） |

**WLAN 起来时内核有一条 WARNING**：`Unbalanced IRQ 375 wake disable`，
出自 `cnss2` 的 `cnss_driver_event_work`，前面是 `msm_pcie_disable` /
`invalid GPIO -22`。是 PCIe 复位期驱动的告警，不是 panic——网卡随后正常起来。
记在这里，将来若真机上 WLAN 偶发不工作，先看这一条。

---

## 四、已经查过、确认没问题的地方

这些是查之前怀疑有问题、查完发现已经做对了的，写下来免得以后重复调查：

| 怀疑 | 实际 |
| --- | --- |
| 手机上有未合并快照时，刷 super 或扩容会毁掉它 | 四处都会拒绝：`install_system`、`update_system`、刷 `ImageKind::Super`、`flash_logical`（`Refusal::Pending`）；脚本安装则在更新未完成时**不映射**该槽的分区 |
| 被拒绝时用户不知道该干什么 | 提示是"{快照状态}。请重启到系统，完成上一次更新后重试"，中英繁体都有 |
| 压缩快照更新失败时界面会让人以为在等 | 状态文案是"此类更新需在系统中合并，启动系统后将自动合并" |
| 配置里点名的库路径漏了 `/odm/lib64` | 查过二进制依赖：`/odm/lib64` 里那几个 `libjc_keymint*` 属于 **StrongBox** 版 keymint，配置启动的是 TEE 版（链接 `/vendor/lib64/libqtikeymint.so`），不需要加 |
| 两个 crate 各有一份"进程还在跑吗" | 确实重复。已合并为 `skyward_sys::running`，并补了真正起进程的测试 |
| 压缩快照读不了时，能不能退回"更新前的内容"先对付用 | **不能**：版本与补丁级别是从挂载分区的 `build.prop` 读的（`installed_props`），退回旧内容会把补丁读旧，keymint 仍会拒密钥，而四个服务却显示"已注册"——比现在的明确拒绝更糟。详见 `docs/vabc-design.md` |

---

## 五、尚未验证的部分

| 项 | 为什么 |
| --- | --- |
| ~~`/data` 在真机上能否解密~~ | **已验证（2026-11-19）**：六个密钥服务注册、`/data` 以 `dm-9` 挂上、477G/366G 真实数据可读 |
| ~~屏幕与背光、触摸驱动、eSE 模块、WLAN~~ | **已验证（2026-11-19，只读）**：屏幕 1440x3200@120 由 Skyward 点起、背光 546、触摸与 eSE 模块加载、`wlan0` 起来且 `wpa_supplicant` 在跑。**触摸是否响应、界面能否操作仍需人看着屏幕** |
| OTA 安装、切槽、开机 | 需要设备，且要冒失败风险 |
| 逻辑分区扩容 | **已验证（只读规划）**：新增 `skyward lp --plan-grow 名字=字节`。对这台手机的真实 super 实测：组上限还有约 762 MB 余量，但**分区表里唯一的空洞只有开头 1 MB**（独立按 `skyward lp` 的 super 扇区号重算，结果与规划器的"没有对齐空闲区"一致）。所以**这台机器上不能就地扩容**，要扩必须先移动已有的 extent——而 `grow` 按设计永不移动它人的扇区。OTA 不受影响：那是 `update_engine` 整体重写元数据并搬 extent |
| 逻辑分区扩容后能否挂载 | 需要设备（`super` 元数据改动不可逆，先备份） |
| sparse super 实际写盘 | 需要设备 |
| ~~触摸是否真的能用~~ | **已验证（用户在有屏幕的机器上试过）**：触摸正常。界面渲染另已用 `skyward capture` 与离线预览比对，差异 6.66/255 |
| ~~压缩快照的读取~~ | **已验证（2026-11-19）**：手机在系统里更新后产生 9 个 lz4 压缩的用户态快照；Skyward 在 B 槽把它们**全部挂载成功**，六个密钥服务全部注册 |
| 压缩快照的合并 | **新增 `skyward merge --compressed`（opt-in，默认仍拒绝）**：直接驱动 daemon 合并，逐分区用 `getstatus,user_snapshot-<分区>` 报真实状态，进度按"已完成分区数"计而不是 daemon 的全局百分比，完成后把 update 状态推到 `MergeCompleted`。**未经真机验证**（需要一次未合并的更新）。原有结论：用 `skyward snapuserd --merge` 逐分区问过 daemon（`getstatus` 要用 `user_snapshot-<分区>` 这个名字，用分区名去问一律返回 failed，那是**名字找不到**而不是真的失败）：**COW 在 super 里的分区能合并完成**（`odm_b`），**COW 只在 `/metadata/gsi/ota/lp_metadata` 镜像里的八个分区停在 `snapshot-merge` 二十分钟不动**，daemon 空闲。最可能的原因是 `init` 第 5 段 `base_path_merge`——它是 daemon 合并时**按路径重新打开 base 设备**用的，现在传的是 dm 设备路径。实测退路可用：从这个半合并状态重启，**系统都在约一分钟内把剩下的合并完** |
| 非 A/B 更新包 | 未实现：xuanyuan 是 A/B 机型，此类包用不上；只加了"包声明了别的机型就拒绝"的安全门 |

---

## 六、真机验证步骤

设备要求：已解锁的小米 15 Ultra，系统 OS4.0.4.0。

**先备份**（不可跳过）：

```
fastboot fetch recovery_a stock-recovery_a.img     # 需要较新的 fastboot
fastboot fetch recovery_b stock-recovery_b.img
```

**刷入**（两个槽位都刷，避免回落到旧的那一份）：

```
fastboot flash recovery_a skyward-v0.3.1-beta2-xuanyuan-OS4.0.4.0.XOACNXM.img
fastboot flash recovery_b skyward-v0.3.1-beta2-xuanyuan-OS4.0.4.0.XOACNXM.img
fastboot reboot recovery
```

**观察点与期望**：

1. **界面出现、触摸可用**。若黑屏，说明 recovery 服务根本没起来——这本身是重要信息，
   请抓 `adb shell dmesg | grep -i skyward`。
2. `adb shell skyward check` 的输出。期望：
   - `the services the key chain needs` 下 keymint / gatekeeper / secure_element / weaver
     都是 `registered`；
   - `the key it keeps:` 后面**不是** `empty`；
   - 表里 `metadata_encryption:` 不是 `(none)`。
3. 首页/设置页里存储容量能显示、`/data` 不再显示"未解密"。
4. `adb shell dmesg | grep -i skyward` 里不应出现
   `devices not brought up` 或 `partitions not mounted`。

**如果 `/data` 解不开，先看这两处再怀疑镜像**：

- `skyward check` 里 `an update in flight:` 是不是 `Snapshotted` 而不是 `None`。
  若手机上有**未合并的压缩快照**，`mount_partitions()` 会**整体失败**——快照无法由
  snapuserd 服务时 `map_snapshot()` 直接报错，而这个错中止整个挂载循环，于是 `vendor`
  没挂上、keymint 起不来、`/data` 解不开。**表现与"镜像坏了"完全一样。**
  处理办法：正常启动一次系统让它合并，再回 recovery 试。
- `skyward check` 里 `the services the key chain needs` 下有没有 `NOT registered`。
  有的话，`/skyward/run/services/<名字>.log` 里就是它退出的原因（缺哪个库、哪个节点）。

**要回报的东西**（一条命令打包）：

```
tools/collect-report.sh report.txt
```

它收 `skyward check`、`dmesg`、`recovery.log`、每个服务的 stderr、mounts、两份 fstab、
属性。若 `adb` 不可用，至少把 `skyward check` 的输出和
`/skyward/run/services/*.log` 发回。

**风险提示**：扩容前请 `dd if=/dev/block/by-name/super of=/sdcard/super_stock.img`
备份；提交元数据不是掉电原子操作。sparse/大镜像写完只 sync 与窗口比对，**刷完请立即重启**。

---

## 七、这份报告的边界

- 所有"实测"都指**在本机对固件文件与代码的核对**，不含真机运行。
- 「配置里某个路径存在」证明的是**文件在**，不证明 HAL 能成功初始化——
  后者只能真机验证。
- 镜像哈希只对应本机这次构建；从 CI 打出会不同（时间戳等），内容应一致。

**`/data` 用户文件（CE）解密的修复**（2026-11-20，真机验证通过）

现象：`/data` 能挂（DE 打开），但用户自己的文件始终关着。日志里的确切原因不是凭据：

```
the key service says a key needs an upgrade
the key service still says a key needs an upgrade after it was started again:
  start a decryption: the service refused the call with its error -62
```

`-62` 是 `KM_ERROR_KEY_REQUIRES_UPGRADE`：用户 synthetic password 的密钥是**旧系统版本**下生成的。原代码遇到它只做一件事——杀掉 keymint 重启再试**同一个密钥**，这对"密钥确实更旧"无效，所以永远第二次同样 -62。

修复：**先真正调用 `upgradeKey`**（带密钥绑定的 app id，与 DE 路径早已在用的调用相同），用续期后的 blob 去开；只有续期被拒时才退回原来的重启重试。续期后的 blob **只留在内存**，不写回手机，系统读到的仍是原样。

真机验证（输入 PIN 后）：

```
skyward: the key service says a key needs an upgrade (...); asking it to renew the key in hand
（此后无任何失败行）

/data/data       → 605 个条目，可读
/sdcard          → DCIM、Documents、Download 等真实内容
user 0 不再出现在"文件仍关着"的列表里（只剩 user 999）
```
**更新页也能合并压缩更新了**（2026-11-20，未在真机验证）

之前 `skyward merge --compressed` 只有命令行能用：更新页对压缩更新只显示一句"需在系统中合并"，
没有可点的合并。现在：

- 更新页在压缩更新（`Standing::Compressed`）时也显示合并滑块，文案改为
  "由后台服务合并，启动系统后也会自动合并"；
- 合并线程在**启动前**一次性判定走哪条路（`standing` 为 Compressed 且快照已映射
  → `run_served`，否则走原内核路径），线程运行期间不再询问屏幕；
- 仅**从页面发起**的合并可能走压缩路径：开机时发现已在合并的那次是系统自己驱动的，
  保持只跟随、不接管。

UI 测试 386 项全过（含新增"压缩更新同样滑动合并"）。
**同步了上游 kirklin/skyward 的三个修复**（2026-11-20）

上游发布了 0.3.1（我们 fork 自 0.2.2，其间上游领先 24 个提交）。其中大部分我们已有
同源提交（cherry-pick 过或独立实现），真正缺的是三个通用修复，已逐个核对后同步：

| 上游提交 | 内容 | 与我们的适配 |
| --- | --- | --- |
| `c5e09f6` | 模块加载后创建的文件要**等它出现**再移交 | 保留我们的 5 秒 `FILES_MADE` 常量；改用上游放在 `skyward-sys` 的 `wait_file`（带超时消息），删掉 main.rs 里的本地重复实现 |
| `37ef67e` | recovery 服务**再次启动时唤醒触摸屏**：面板先休眠再用暗帧唤醒，控制器休眠时不响应设置 | 合入唤醒序列；保留我们的双击唤醒参数（`double_tap`，本机配置在用）；去掉上游的 `eprintln` 调试行 |
| `2ea0065` | 电池/系统属性**读取失败时页面保持**，不整页崩溃 | 全部合入；冲突中被吞掉的 `ToldOnce` 测试已手动补回 |

**没有合并的部分**（有意）：

- 上游 0.3 的 UI 重构（`app.rs` 拆成 27 个模块）：与我们 355 个提交深度冲突，
  机械合并必然丢改动；要跟需要一次专门的重定向工作，不在这次范围
- `devices/` 其他机型配置：恢复上游的 `dada/ishtar/nezha` 三个文件（它们带修复），
  我们的 `xuanyuan.toml` 不受影响
- slides/ 文档图片：上游新增，无冲突，随后续合并带上
**OTA 安装后的"卡米"问题**（2026-11-20，真机实测）

**现象**：在 Skyward recovery 里用 `update_engine_sideload` 安装完整包（10.4 GB），
安装本身**成功**（`exit status: 0`，九个分区写入并校验通过），但重启后**卡在小米 logo**，
无法进入系统，最终回落 fastboot。A/B 两槽的 `slot-successful` 都变为 `no`，
`slot-retry-count` 耗尽。

**根因**（已定位）：

1. update_engine 把 A 槽（next 槽）的 **boot、vbmeta、recovery 全部刷成原厂**；
2. Skyward 的"写回 recovery"（`reflash`）**失败**：`write recovery_a: Operation not permitted`——
   不是简单的写保护，而是**底层存储/内核拒绝**（discard 和 write 都报 EPERM）；
3. 这个失败**没有阻止安装完成**（`Done::Installed` 仍返回），但 recovery 实际没写回；
4. **AVB 校验**（`post_avb`）只在 `disable_avb` 选项开启时才执行，且只改 vbmeta flags——
   **小米 OS 4.0 的 bootloader 有额外的签名校验**，即使 `disable-verification` 置位，
   仍然拒绝启动非原厂签名的 boot。

**结论**：这是**小米 OS 4.0 的硬件级限制**，不是 Skyward 的 bug。
网上同类报告（[知乎](https://wenku.csdn.net/answer/31fxgsm7d5mi)、
[看雪](https://bbs.kanxue.com/thread-265792.htm)）都指向同一机制：
**第三方 recovery 刷入完整包后，boot/vbmeta 被重置为原厂，而小米 bootloader
在 OS 4.0 之后不再接受 `disable-verification` 标志的绕过**。

**Skyward 可以改进的**：

- recovery 写回失败时，UI 应明确警告"**下次启动可能失败**"，而不是只显示一行小字；
- 如果 `disable_avb` 开启，应在 recovery 写回失败后**仍然尝试禁用 AVB**
  （当前代码在 `reflash` 失败后会继续执行 `post_avb`，但用户可能没开这个选项）。

**用户应对**：

- 安装 OTA 前，**备份原厂 boot 和 vbmeta**；
- 安装后如果卡米，进 fastboot 刷回备份的 boot/vbmeta；
- 或者**不要从第三方 recovery 安装完整包**——只在系统内更新（设置→系统更新），
  由小米自己的更新器处理 AVB。

**未验证的假设**：如果安装前 `disable_avb` 已开启且 vbmeta 的 `disable-verification`
已置位，是否仍会被拒绝？需要一次带这个前提的实测。

---

**卡米后的完整救回路径**（2026-11-21 真机走通，记下来备用）

实测从"两槽 successful=no、retry 耗尽、连 recovery 都进不去、只回 fastboot"
到系统完全恢复：

1. `fastboot flash boot_a <root 备份>`、`boot_b` 同一镜像——**boot 是关键**，
   vbmeta 不必刷（解 bootloader 上对 boot 的校验靠的是刷入与 vbmeta 匹配过的镜像，
   或直接刷回曾经启动过的那份）；
2. `fastboot set_active a` 重置槽计数；
3. `fastboot reboot` 进系统——**15 秒起机**，`/data` 477G 完整、607 个应用数据在；
4. root 由 Magisk 备份 boot 带回：`context=u:r:magisk:s0`。

**另一个实测发现**：recovery 刚启动时 `skyward check` 可能报
`NOT registered: keymint`——那是服务重启中的暂时态，几秒后重跑即注册。
**check 跑太早会误报**，不是缺陷。
**"wordmark 的 y 渲染错误"已修**（2026-11-21，真机截图定位）

用户在真机上看到 "Skyward" 的 y 上盖着一块矩形色块。此前我两次离线测量都得出"正常"，
**那是错的**——第一次用了 ppd=1.0（字号 37.8px），而真机是 37.8dp × 3.75 = **141.75px**，
bug 只在足够大的字号下出现。

**决定性证据链**：

1. 真机截图与深色预览**逐像素一致**（差异 0 像素）→ bug 可离线复现，是渲染代码；
2. 同字号下**纯文字渲染正常** → 字形光栅化本身没问题；
3. sprite 覆盖率图里 y 背后有**竖直的矩形半透明带**；
4. 逐偏移扫描：**qx=3（水平 0.75 像素）时 ink 缓冲 8991 像素里只有 19 个空的**，
   整个包围盒被糊满，最大覆盖率只有 0.712（正常应有 1.0）；
5. 字号扫描：**≤130 px 正常，≥132 px 出错**；
6. 同字号下**静态字体（KirkMono）正常，可变字体（MiSans）出错**
   → ab_glyph 对可变字体在大字号 + 分数位置的光栅化缺陷。

**修复**：`Face::on_whole_pixels` —— em ≥ 120 px 的字体在**水平方向按整像素定位**
（此时 1/4 像素不可见），更小的字号保持原有分数定位，所以其它文字一字未变。

**修复后实测**：wordmark 墨迹占框比 34.4% → 30.6%；k 不再带着它本没有的降部；
y 旁边的孤立碎片消失；对比图里 y 完整、矩形色块消失。

**回归测试**：`every_letter_is_drawn_at_every_quarter_of_a_pixel` —— 对 wordmark 的
每个字母、每个 1/4 像素位置各画一次，要求**既要有实心笔画**（否则是被糊平），
**空像素又要过半**（否则是被填成方块）。这个断言正是从外部看这个 bug 的样子。