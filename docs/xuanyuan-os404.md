# 小米 15 Ultra（`xuanyuan`）与澎湃 OS 4.0.4 适配说明

这份说明写的是本分支相对上游 `kirklin/skyward` 的改动、已经验证到什么程度、
以及哪些事还没有做。**没有真机验证的部分一律不算完成。**

## 一、`/data` 解密

底包与解密链来自 `zhouzt0121` 的 `origin/os404-adaptation` 分支，本分支另外做了四件事：

1. **交接节点前先等它出现**（`crates/skyward/src/main.rs`）
   - 驱动是在自己的线程里 probe 的，`finit_module` 返回时它要交出去的节点往往还没建好。
     Android 17 上 `stm_st54se_gpio` 就是这样：`chown` 以 `ENOENT` 失败，
     而旧代码一失败就中断整条密钥启动流程，`keymint` / `gatekeeper` 根本不会启动
     （就是上游 issue #55 的形态）。
   - 现在 `hand_over` 对每个待交接节点最多等 5 秒（`wait_for_file`）再改属主与权限。
   - 注意：`xuanyuan` 的配置里**没有** `[[owners]]`。安全元件服务以 root 运行，
     root 不受节点属主限制，出货镜像也没有为它加这条；而 `hand_over` 失败是致命的，
     为一个没人需要的节点加这个风险不划算。见[第四节](#四与出货镜像的对照)。

2. **Binder 调用超时**（`crates/skyward-binder/src/driver.rs`）
   - 安全服务在拿不到安全元件时会一直不回答，驱动默认要等几十秒，界面就卡在解密页。
   - 移植 `transact_within`：在写端之外用 `poll` 等驱动的可读性，weaver 调用给 4 秒上限。
   - 与本分支已有结构（`Waiting` / `exchange` / `take_in`）合并：只有在**没有待告知驱动内容**
   时才先等可读，否则会与“先回执才能继续”的调用互相等死。

3. **服务的错误输出不再丢弃**（`crates/skyward/src/services.rs`）
   - 之前每个服务都以 `stderr(Stdio::null())` 启动，于是
     `CANNOT LINK EXECUTABLE … library "libvndksupport.so" not found` 这种唯一能定位问题的
     一行被直接丢掉，日志里只剩「服务结束了」。上游 issue #64 是靠手动复现才找到缺库的。
   - 现在写到 `/skyward/run/services/<程序名>.log`（追加，超过 64 KiB 时清空重写），
     建立不了就退回 `null`，任何情况都不会让服务因此启动失败。
   - 排查时直接：`adb shell cat /skyward/run/services/android.hardware.security.keymint-service-qti.log`

4. **一条命令产出诊断**（`skyward check`，`crates/skyward/src/main.rs`）
   - 上面那些信息分散在 fstab、binder、几个日志文件里，真机排查要在 adb 里敲七八条命令。
   - `adb shell skyward check` 一次打印：底包版本、槽位、已装系统版本、phone 自己的
     `/data` 表项（`metadata_encryption` / `keydirectory` / `fileencryption`）、
     密钥文件是否存在（**只打印大小，不打印内容**）、密钥链各服务是否已注册、
     是否有未合并的更新、以及每个服务自己错误的最后 12 行。
   - 全部只读；每一项单独探测，探不到就写「探不到」，不会因为一项失败就停下。
   - 电脑上还有 `tools/collect-report.sh`：它把 `skyward check`、`dmesg`、
     `recovery.log`、每个服务的日志、mounts、vendor/odm fstab、属性、模块列表
     一次收进一个文本文件（**不含密钥内容**，密钥只列文件名和大小），适合直接贴出去。

上游 issue #64 记录的直接原因是 `keymint` 缺 `libvndksupport.so`；适配分支已经通过
`provide_system_libraries()` 从已安装系统补 `libvndksupport.so` 与 `libdl_android.so`，
并给服务补 `/vendor/lib64/hw` 等库路径。本分支沿用该做法，没有扩大范围。

### 已经核对过的证据

这几条是拆开真实镜像查的，不是推测：

1. **底包 recovery 确实缺那几个库。** 4.0.4.0 底包 ramdisk 里有 72 个
   `system/lib64/*.so`，但 `libvndksupport.so`、`libdl_android.so`、`libhardware.so`、
   `libqtigatekeeper.so`、`libEseUtils.so` **一个都没有**。issue #64 的机制成立。
   `provide_system_libraries()` 补的正是前两个；`libhardware.so` 在 `/vendor/lib64`、
   后两个在 `/vendor/lib64/hw`，都由服务的库路径覆盖。

2. **AVB footer 缺 `security_patch` 不是 4.0.4 的退化。** 我把 4.0.0.18 和 4.0.4.0
   两个底包的 recovery 分区都取出来（各自哈希都与发布配置一致），解析各自的 vbmeta：
   **两者都只有 `com.android.build.recovery.fingerprint`**，都没有 `security_patch`、
   也没有 `os_version`；而出货的 v1.0.0 镜像 footer 里一个属性都没有。
   所以打包时那条 `names no security patch in its footer` 警告是这个机型的常态，
   不必再往这个方向查。

3. **底包 recovery 的版本属性是过时的，必须由运行时覆盖。** 4.0.4.0 底包的
   `prop.default` 写的是 `ro.build.version.sdk=35`、`release=15`
   （即 Android 15 的数），而系统本身比这新。打包器把
   `ro.build.version.release` / `release_or_codename` / `security_patch` /
   `ro.vendor.build.security_patch` 四个属性从 `prop.default` 里删掉，
   由 `adopt_installed_version` 在启动时用**已安装系统** build.prop 的值写入，
   再由 `environment::pin()` 重启 keymint 让它重新读取。
   两个底包在这四个属性上的差别也确认了：4.0.0.18 是 `2026-09-01`，4.0.4.0 是
   `2026-10-01`。

4. **配置里点名的内核模块在固件里都真实存在。** 从 4.0.4.0 底包里取出
   `vendor_dlkm`（87,343,104 字节，391 个模块）与 `system_dlkm`（15,282,176 字节）
   挂载后逐个核对：

   | 模块 | 在哪 |
   | --- | --- |
   | `synaptics_tcm2`（触摸） | `vendor_dlkm/lib/modules/` |
   | `stm_st54se_gpio`、`stm_nfc_i2c`（eSE） | `vendor_dlkm/lib/modules/` |
   | `qca_cld3_peach_v2`（WLAN） | `vendor_dlkm/lib/modules/` |
   | `rfkill`（cfg80211 要的） | `system_dlkm/flatten/lib/modules/` |

   五个都不在 `modules.blocklist` 里，两个 peach 驱动都在 `modules.load` 里。
   打出来的镜像根目录确实有 `vendor_dlkm` 与 `system_dlkm` 两个目录——
   `skyward_vab::partitions::mount_point` 会把分区挂在根目录同名目录上，
   而 `skyward_modules::source_dir` 找的是 `/vendor_dlkm/lib/modules` 与
   `/system_dlkm/flatten/lib/modules`，两边对得上。

   加载器要读的依赖表也在：`vendor_dlkm/lib/modules` 有 391 个 `.ko` 与
   `modules.dep`/`modules.alias`/`modules.softdep`/`modules.load`；
   `system_dlkm` 里**空的**是 `lib/modules`，有内容的是
   `flatten/lib/modules`（96 个 `.ko` + `modules.dep`），配置选的正是后者。

5. **模块或服务起不来不会让 recovery 起不来。** `prepare()` 把 super 挂载与
   `bring_up` 的失败都写进内核日志（"partitions not mounted" /
   "devices not brought up"），界面照常显示。所以这台的失败表现应该是
   "密钥服务未注册"，而不是黑屏——`skyward check` 正是为此写的。

6. **配置里指向 `/vendor` 的每一条路径都在真实固件里。** 挂载 4.0.4.0 的 `vendor`
   分区（742,723,584 字节）后逐个核对：配置启动的九个程序
   （`init.kernel.post_boot.sh`、`qseecomd`、`ssgtzd`、`hlosminkdaemon`、
   keymint / gatekeeper / secure_element / boot 四个 HAL、`wpa_supplicant`）全在；
   `/vendor/lib64/libhardware.so` 与 `/vendor/lib64/hw` 在；两个 WLAN 配置文件
   （`peach`、`peach_v2`）在；表里 `persist` 与 `modem` 也都写着。

   而 `libvndksupport.so` **不在 vendor**，也不在 recovery 自己的
   `/system/lib64`（72 个库里没有它）——它只在**已安装系统**的 `/system/lib64`。
   所以 `provide_system_libraries()` 从系统分区补它是唯一的办法，这也把
   issue #64 的机制闭合了。

7. **weaver 与 keymint 的库路径也是对的。** 取出 4.0.4.0 的 `odm`
   （4,563,202,048 字节）核对：

   - `/odm/bin/hw/android.hardware.weaver-service.thales` 存在；
   - 它 `NEEDED` 里的 `ese_weaver_thales.so` 与 `mi_weaver.so` 都在
     `/odm/lib64`，而配置正好给了 weaver 服务
     `["/system/lib64", "/vendor/lib64", "/odm/lib64"]`；
   - `/odm/lib64` 里那几个 `libjc_keymint*.so` 属于**另一个** keymint
     （`/odm/bin/hw/android.hardware.security.keymint-service.strongbox-thales`），
     不是配置启动的 TEE 版（`/vendor/bin/hw/android.hardware.security.keymint-service-qti`，
     它链接 `/vendor/lib64/libqtikeymint.so` 且在 vendor 下的依赖齐全）。
     所以 keymint 服务的库路径**不需要**加 `/odm/lib64`——这一点是查过才没有乱加的；
   - `libqtigatekeeper.so` 与 `libEseUtils.so` 确实在 `/vendor/lib64/hw`，
     而 gatekeeper 与 secure_element 两个服务的库路径里都有它。

8. **要补的两个库在已安装系统里确实存在，而且不在同一个目录。** 取出 4.0.4.0 的
   `system`（977,719,296 字节，925 个 `.so`；recovery 自己的 `/system/lib64` 只有 72 个）：

   | 库 | 在已安装系统里的位置 |
   | --- | --- |
   | `libvndksupport.so` | `/system/lib64/libvndksupport.so`（真文件，51,504 字节） |
   | `libdl_android.so` | `/system/lib64/libdl_android.so` 是**符号链接**，指向 `/apex/com.android.runtime/lib64/bionic/libdl_android.so`；真实副本在 `/system/lib64/bootstrap/libdl_android.so`（34,848 字节） |

   第二行**在真机上核对过**：`adb shell ls -l /system/lib64/libdl_android.so` 给出那条链接。
   早先只看镜像时把它记成"只在 bootstrap"是错的——`[ -e 路径 ]` 会跟随符号链接，
   而镜像里 APEX 没挂载，链接是悬空的，于是被判成不存在。

   `provide_system_libraries()`（`crates/skyward/src/secure.rs`）逐个库依次在
   `/mnt/system/system/lib64` 与 `/mnt/system/system/lib64/bootstrap` 里找，用的是
   `is_file()`：APEX 挂着时取 `lib64` 那份，没挂时链接悬空、自动落到 `bootstrap` 那份。
   两条路都取得到，这正是**按顺序查两个目录**而不是各取一次的原因。

9. **keymint 的依赖闭包已经逐个查过，缺的正好就是那两个库。** 对着真实的三棵树
   （recovery ramdisk 的 `/system/lib64` 72 个库、`/vendor/lib64`、已安装系统的
   `system/lib64`）逐条核对 keymint 服务的每个 `NEEDED`：

   - 直接依赖全部落在 **ramdisk 或 vendor**：`liblog`/`libcutils`/`libbase`/`libutils`/
     `libbinder_ndk`/`libhidlbase`/`libcrypto`/`libc++`/`libc`/`libm`/`libdl` 在 ramdisk；
     `libhardware`、`libqtikeymint`、`android.hardware.keymaster@4.0`、
     `android.hardware.security.keymint-V3-ndk` 等**只在 vendor**（配置给了 `/vendor/lib64`）；
   - 断链在**传递依赖**：

     ```
     android.hardware.security.keymint-service-qti (vendor)
       └── libhardware.so (vendor)
             └── libvndksupport.so   ← vendor 没有，ramdisk 没有，只在已安装系统
                   └── libdl_android.so ← system/lib64 是链接进 runtime APEX；真副本在 bootstrap/
     ```

   这就是 `provide_system_libraries()` 只补这两个库的理由：其余依赖本就在手边，
   补多了反而会掩盖真正缺的那一环。

10. **这台机器的整张表，Skyward 的解析器读得下来。** 把 4.0.4.0 的
    `/vendor/etc/fstab.qcom` 整份喂给 `skyward_block::fstab::entries`：

    | 项 | 结果 |
    | --- | --- |
    | 非空非注释行 | 62 |
    | 解析出的条目 | **62**（一条没丢） |
    | `/data` 条目 | 2 条（f2fs 与小米的 mifs） |
    | `metadata_encryption` | `aes-256-xts:wrappedkey_v0` |
    | `fileencryption` | `aes-256-xts:aes-256-cts:v2+inlinecrypt_optimized+wrappedkey_v0` |
    | `keydirectory` | `/metadata/vold/metadata_encryption` |

   表里确实把 `/vendor_dlkm`、`/system_dlkm` 挂在自己的同名目录上，与
   `mount_point()` / `source_dir()` 的假设一致。这两行真机文本已经作为回归用例
   进了 `crates/skyward-block/src/fstab.rs`（`the_table_the_phone_ships_is_read_as_the_phone_means_it`）。

11. **要加载的模块，连依赖文件都齐。** 用固件自带的 `modules.dep` 解析每个模块的
    **传递依赖**，再逐个确认 `.ko` 文件真的存在：

   | 配置要加载的 | 连依赖一共 | 文件 |
   | --- | --- | --- |
   | `synaptics_tcm2` | 5 | 全在 |
   | `stm_st54se_gpio` | 1 | 全在 |
   | `stm_nfc_i2c` | 5 | 全在 |
   | `rfkill` | 1 | 全在 |
   | `qca_cld3_peach_v2` | 31 | 全在 |
   | `qca_cld3_peach` | 31 | 全在 |

   真机的依赖行也已作为回归用例进 `skyward-modules`（触摸驱动必须排在它依赖的
   四个之后、`stm_st54se_gpio` 没有依赖、eSE 驱动带出 `qseecom_proxy` 等四个）。

## 二、逻辑分区扩容（需要改 super 元数据）

`crates/skyward-lp`：

- `encode_metadata` 按**源元数据自己的版本**写回（1.0/1.1 的 128 字节头、1.2 的 256 字节头），
  retrofit 的 `_a` / `_b` 名称与 `ATTR_SLOT_SUFFIXED` / `GROUP_SLOT_SUFFIXED` /
  `BLOCK_DEVICE_SLOT_SUFFIXED` 标志在解析时被剥掉，编码时重新加回。
- `commit` 写目标 metadata slot 的**主备两份**，每份 `sync_data` 后回读比对，最后 `sync_all`。
- `grow` / `grow_avoiding`：只追加新的 `Linear` extent，从不移动、改写既有 extent；
  按设备 `alignment` / `alignment_offset` 对齐，跳过元数据区与已占用区间，
  检查组上限、设备边界、分区之间不重叠。

**一个必须说明的正确性点**：super 里同时放两个槽位的分区，而每个槽位的元数据只描述自己那份。
于是单看一份元数据去扩容，新 extent 可能正好压在另一槽位的分区上。
`grow_avoiding` 允许把“别人占用的扇区”传进来一起避开；`grow_across` 把多个槽位放在一起规划：
每个槽位都避开其它槽位的既有 extent 与之前已生成的计划，最后再整体校验一遍。
双槽规划放在 `skyward-lp` 里而不是安装流程里，就是为了能在电脑上测。

安装流程（`crates/skyward-install/src/run.rs::flash_logical`）：

1. 先算稀疏镜像展开后的大小；
2. 对所有选中的槽位**逐一规划**（不写盘）；
3. 合并校验两槽不重叠；
4. `detach_once` 后卸载各槽位的映射；
5. 提交元数据；
6. 重新映射为可写的 `skyward-flash-<base><suffix>`；
7. 写入并 `fsync`；
8. 卸载映射。

镜像大于分区时不再直接拒绝，而是按上面的流程用 super 的**空闲空间**扩容；
没有空闲空间或与另一槽冲突时，在写第一块盘之前就失败。

写入时“这是分区镜像还是文件系统镜像”是**显式传入**的（`image::Purpose`），
不是从设备路径猜的：逻辑分区映射出来的节点是 `/dev/block/dm-N`，
按名字判断会失效。这个区分决定两件事——签名镜像的 AVB footer 要不要搬到分区末尾，
以及写之前能不能先 discard 这一段。

## 三、sparse super 的并行写入

`crates/skyward-install/src/sparse.rs`（新文件）：

- **先完整预检再开设备**：`plan()` 校验魔数、版本、头长度、块大小、
  每个 chunk 的 `total` 与 payload 长度、`total_blocks` 与展开偏移、
  尾部多余字节。任何一处不对都在写盘前失败。
- **DON'T CARE 一律保留**：只 `seek` 过去，不清零、不 discard。
  这是 sparse super 的关键——没被 chunk 覆盖的区域属于 super 里其它分区（包括另一槽位）。
  因此这条路径**绝不**做整盘 discard。
- **有界流水线**：4 个 4 MiB 缓冲、一个读线程预读 RAW/FILL、主线程写；
  跨缓冲区的大 chunk 按实际偏移续读，FILL 的 4 字节 pattern 按 `consumed % 4` 续相位。
- **CRC32 记录按字节校验**：DON'T CARE 区间以零参与 CRC（与 AOSP 语义一致），
  但**不**写到设备上。CRC 不符会让整次写入失败。
- 写完 `sync_all`。

正确性上曾经踩到的坑（留在这里免得再犯）：
读线程在 CRC 不符时会提前返回，于是空闲缓冲通道被关闭；写线程回收缓冲失败后
直接 `break`，把“失败”走成了 `Ok(())`。现在回收失败不再结束循环，
已排队的数据照常取完并判定。

识别上，sparse 镜像按**展开后**的内容分类：展开到 `SUPER_GEOMETRY_AT` 处是
super 的 geometry 魔数就是 `super`，否则是逻辑分区文件系统镜像。
这个判定放在 `sparse::expands_to`，因为它只读覆盖该偏移的那一个 chunk 的四字节，
不必展开整个镜像，也就能在电脑上单独测。
erofs/f2fs 的超级块在偏移 1024，ext4 的魔数在 1080。

## 四、与出货镜像的对照

`zhouzt0121` 那个 Release 的实际镜像 `skyward-v1.0.0-xuanyuan-OS4.0.0.18.XOACNXM.img`
（100 MiB，SHA-256 `f797dcfa…`）是可以直接拆的：它是 v4 boot image，ramdisk 是
LZ4 legacy 帧 + newc cpio。把它 `skyward/device.toml` 解出来后与本分支逐行 diff，
**只有两处不同**：

| 项 | 出货版 | 本分支 |
| --- | --- | --- |
| `[stock]` | `OS4.0.0.18.XOACNXM` | `OS4.0.4.0.XOACNXM`（哈希已核，见上） |
| 其它 | — | 多一段说明注释 |

也就是说服务链（10 条）、WLAN、`[firmware]`、`[partitions] mounted`、`[logd]`、
`[touch]`、`[display]` 与出货配置**完全一致**。

对照过程中我删掉了自己先前加的一条 `[[owners]]`（把 `/dev/st54spi_gpio` 改成
`system:system 0660`）。理由是三条：出货配置没有它；安全元件服务本身以 root 运行，
root 不受属主限制；而 `hand_over` 失败会中止**整条密钥启动流程**——这正是上游
issue #55 的故障形态，为一个没人需要的节点加这个风险不划算。

同一份 ramdisk 里还能看到两件事：

- **没有 `snapuserd`**，`/snapuserd` 相关条目为 0，只有原厂的
  `system/bin/update_engine_sideload`。也就是说压缩快照这个缺口，出货版同样没有解决，
  不是本分支独有。
- `system/etc/recovery.fstab`（出货版用的那份）里 `/data` 一行**没有
  `metadata_encryption=`**，只有
  `fileencryption=ice,wrappedkey,keydirectory=/metadata/vold/metadata_encryption`。
  但 Skyward 运行时读的是手机的 `/vendor/etc/fstab.qcom`
  （见 `skyward_block::fstab::FILES`），不是这份 recovery fstab。

  **这一条已经查实**：从 4.0.4.0 底包里取出 `vendor` 分区（742,723,584 字节）挂载后，
  真实的 `/vendor/etc/fstab.qcom` 第 88 行是：

  ```
  /dev/block/bootdevice/by-name/userdata /data f2fs ... latemount,wait,check,formattable,
    fileencryption=aes-256-xts:aes-256-cts:v2+inlinecrypt_optimized+wrappedkey_v0,
    keydirectory=/metadata/vold/metadata_encryption,
    metadata_encryption=aes-256-xts:wrappedkey_v0,
    quota,reservedsize=128M,sysfs_path=/sys/devices/platform/soc/1d84000.ufshc,checkpoint=fs
  ```

  所以这台**用的是 metadata encryption，且密钥是包裹（wrapped）的**：
  FBE v2 + inlinecrypt，元数据密钥由 `wrappedkey_v0` 包着，解包要走 keymint，
  而锁屏密码的保护走 weaver。**这解释了“密钥服务起不来 → /data 打不开”为什么成立**：
  不是少了某条挂载标志，而是没有可用的包裹密钥。上游 issue #64 补
  `libvndksupport.so` 让 keymint 能起来，与本分支的链正好对上。
  `keydirectory` 也与 `skyward_crypto::data::key_dir` 读到的一致。

  同一份表里还有第二条 `/data`（类型 `mifs`，小米自己的文件系统），
  `fstab::read("/data")` 取第一条，也就是 f2fs 那条。

## 五、验证到什么程度

> 汇总版（含真机验证步骤与要回报什么）在 `docs/xuanyuan-verification.md`。

已经跑过、可以复现的：

| 范围 | 命令 | 结果 |
| --- | --- | --- |
| 整仓测试（CI 跑的就是这条） | `cargo test --workspace --locked` | 全部通过，0 失败（45 个结果块） |
| 整仓 lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` | 退出码 0 |
| `skyward check` 的报告措辞 | `cargo test -p skyward --bin skyward` | 9 项通过 |
| 构建镜像 | `skyward-pack stock` + `skyward-pack recovery` | 通过，104,857,600 字节 |
| Android 服务构建 | `cargo ndk -t arm64-v8a --platform 30 build --locked --release -p skyward` | 退出码 0 |
| 两个静态首程序 | 同上加 `RUSTFLAGS=-C target-feature=+crt-static` | 退出码 0 |
| 打包器 | `cargo build --locked --release -p skyward-pack` | 退出码 0 |
| LP 读取/编码/扩容/提交 | `cargo test -p skyward-lp` | 23 项通过（原有 7 项、扩容与双槽规划 16 项） |
| sparse 解析与写入 | `cargo test -p skyward-install sparse` | 11 项通过（RAW/FILL/DON'T CARE 混合、坏 CRC 拒绝、格式错误不改盘、多段空洞、super 判定、头扩展与尾部填充、chunk 数上界、跨缓冲的大 chunk、12 轮确定性随机构造、空洞也计入进度、设备中途拒写不死锁） |
| Android 全工作区编译 | `cargo ndk -t arm64-v8a --platform 30 check/clippy --workspace --all-targets` | 通过，零告警 |

前三项是在 Linux（WSL 里的 Ubuntu 24.04，rustc 1.100.0）上跑的；为此在本机手工导入了
一个 `ubuntu-base` 24.04 rootfs 并装了 NDK r29 与 `cargo-ndk 4.1.2`。
Windows 主机上跑不了整个 workspace：`skyward-bootimg` 的 ramdisk 解包用的是 Unix 专有接口。

另外两项构建输入的核对：

| 项 | 做法 | 结果 |
| --- | --- | --- |
| `devices/xuanyuan.toml` 能被真实解析器读入 | `cargo test -p skyward-config` | 29 项通过（新增 1 项专测 xuanyuan 配置） |
| `[stock] recovery` 的 SHA-256 与固件一致 | 从 `payload.bin` 清单里读 recovery 分区的声明哈希 | 一致（`55f49105…7fea27b`） |

第二项是这样做的：底包 zip 有 9.7 GB，但 `payload.bin` 以 Stored 方式存放，
所以按 HTTP Range 只读了它的清单（约 370 KB），解析 protobuf 取出
`partitions[name="recovery"].new_partition_info.hash` 与配置比对。
底包地址本身返回 `200`、`Content-Length` 10382731686 字节。

**没有验证的**：真机上的 `/data` 解密、OTA 安装、扩容后 mount、sparse 实际写盘。
这些都需要一台已解锁的 xuanyuan，本环境做不到。
所以本分支**不宣称**“已完全适配”，只提供可编译、测试通过、镜像已实际构建并核对过内容、
且失败会安全中止的实现。

### 构建出来的那份镜像

```
skyward-v0.3.1-beta2-xuanyuan-OS4.0.4.0.XOACNXM.img
104,857,600 字节
SHA-256 84bf36e7d2431034035948faf5d0243f9ce150d596a2d3dbd7f670f8ae22000c
```

打包器自己从 4.0.4.0 更新包里取出 recovery 分区并核对了哈希。镜像内容逐项核对过：
形状与底包一致（`ANDROID!`、header_version 4、kernel 0、同尺寸）、
`system/bin/recovery` 是 Skyward 本体（8996696 字节）、`system/bin/skyward` 与五个
脚本命令链接都在、`skyward/device.toml` 与仓库里的逐字节相同、OMAPI 的二进制与
VINTF 清单齐全、`prop.default` 末尾是 `ro.secure=0` / `ro.debuggable=1` 且已移除构建
版本属性。**它没有在任何手机上运行过。**

对它做了一次结构审计（`tools/audit-image.py`，32 项检查全过）——查的都是 bootloader
在运行任何代码之前会读的东西：

| 检查 | 结果 |
| --- | --- |
| boot 头 | `ANDROID!`、v4、头长 1584、kernel 0、ramdisk 落在界内 |
| ramdisk | LZ4 legacy 解出 1069 个 newc 条目，以 `TRAILER!!!` 结束 |
| 关键条目 | `system/bin/recovery`（AArch64 ELF）、`system/bin/skyward` → `recovery`、`skyward/device.toml`、`system/bin/se_omapi` |
| AVB footer | `AVBf`，声明的镜像长度 51,261,440（补页后），指向 `AVB0` |
| 哈希描述符 | 覆盖该长度、名字 `recovery`、算法 `sha256`、**摘要等于 sha256(salt + 镜像)** |
| 属性描述符 | `com.android.build.recovery.fingerprint` |

顺带确认了一件本来会悬着的事：打包器写出的哈希描述符布局与**底包自己完全同构**
（载荷从 132 字节开始、名字长度在 56），并不是通用 libavb 头里那套 16 字节算法 +
60 个 8 字节保留字的排法。底包与出货镜像都是这个形状，所以这不是 bug；这个形状已用
`crates/skyward-pack/src/avb.rs` 里的一个测试钉住，免得以后被"顺手改规范"。

## 六、明确没有做的部分

> 压缩快照这一项另有单独一份现状与方案：`docs/vabc-design.md`（含实测到的内核与守护进程情况）。

1. **recovery 内应用/合并压缩快照（VABC）**
   原厂 recovery 版 `update_engine_sideload` 在 recovery 中直接把
   `GetVirtualAbCompressionFeatureFlag()` 判为 NONE（AOSP 注释就写着
   “Don't attempt VABC in recovery”），增量 payload 还会在 `DeleteSourcePartitions()`
   处直接报错。也就是说：**打开包检查的开关并不能让压缩快照工作**。
   实测 4.0.4.0 的 `system` 分区里 `system/bin/snapuserd` 与
   `system/lib64/libsnapshot.so` **都在**，缺的不是手机上的东西，而是 recovery 侧那套
   （`update_engine_sideload` 在 recovery ramdisk 里，且它自己把 VABC 关掉了）。
   要在 recovery 内做这件事，需要与固件匹配的 `libsnapshot` / `snapuserd.recovery`、
   `dm-user` 设备表与两阶段合并，属于另一次 AOSP 侧的工作量，不是几十行 Rust 能补的。
   因此这里保持**显式拒绝**（`Refusal::Incremental`，以及 `snapshots.rs` 对
   `using_snapuserd` 的 `ensure!`），失败原因写在日志里，不做半吊子处理。

   顺带说明现有能力边界：**完整包（非增量）走原厂引擎在 recovery 内写的是内核
   `dm-snapshot` 快照**，这条路径 Skyward 本来就能映射与合并，压缩快照这一缺口不影响它。

2. **非 A/B 机型的更新包**
   `xuanyuan` 是 A/B + 动态分区机型，运行别人的非 A/B 包没有意义，所以这里不做
   "让非 A/B 包在本机跑通"。真正相关的是**别让不该跑的包跑起来**，本分支补了一半：

   - BLOCK/AB 类型的 `payload.bin` 包本来就有 `NotAb` 拒绝，保持不变；
   - **带安装脚本的 zip 原先完全不看设备**：它有自己的一套 updater，直接写本机分区，
     却没有 `pre-device` 校验。现在 `package::declares_other_device` 读它的
     `META-INF/com/android/metadata`，只要写了 `pre-device` 且不包括本机，就在
     用户确认之前拒绝（`Refusal::OtherDevice`）；没写、或本机连
     `ro.product.device` 都读不出来时，行为和以前完全一样。

   还没做的是：真正让一个非 A/B 包在这台 A/B 机上按它的布局跑起来（`update-binary`
   的无后缀分区映射、`/cache/recovery` 兼容）。那对 xuanyuan 没有用途。

3. **真机回归**：见下。

## 七、构建

与上游一致，用 Linux 或 WSL（Windows 上 `cargo ndk` 需要 NDK + 可用的 host 链接器）：

```sh
cargo ndk -t arm64-v8a --platform 30 build --release -p skyward
RUSTFLAGS="-C target-feature=+crt-static" \
  cargo ndk -t arm64-v8a --platform 30 build --release \
  -p skyward-system-init -p skyward-permissive-init --target-dir target/system-init
cargo build --release -p skyward-pack

target/release/skyward-pack stock --config devices/xuanyuan.toml --out stock/recovery.img
target/release/skyward-pack recovery \
  --stock stock/recovery.img \
  --binary target/aarch64-linux-android/release/skyward \
  --system-init target/system-init/aarch64-linux-android/release/system-init \
  --permissive-init target/system-init/aarch64-linux-android/release/permissive-init \
  --config devices/xuanyuan.toml \
  --font resources/fonts/MiSansVF.ttf \
  --mono resources/fonts/KirkMono-Regular.ttf \
  --mono-license resources/fonts/KirkMono-OFL.txt \
  --omapi resources/omapi \
  --out out/skyward-xuanyuan-OS4.0.4.0.img
```

`.github/workflows/build-xuanyuan.yml` 已经按这套流程构建并校验原厂 recovery 的 SHA-256。

### 没有 Linux 环境时怎么拿到镜像

不用本地搭 NDK：把本分支推到自己的 fork，仓库自带的
`.github/workflows/build-xuanyuan.yml` 会

1. 在 `ubuntu-24.04` 上跑 `cargo test --workspace` 与 `clippy -D warnings`；
2. 用 `cargo ndk` 构建 `skyward`、静态的 `system-init` / `permissive-init`；
3. 用 `skyward-pack` 从底包链接里只取 recovery 分区，并按 `devices/xuanyuan.toml`
   里的 `[stock] recovery` **核对 SHA-256**（对不上直接失败，不会拿错底包）；

## 八、真机验证清单

按顺序做，每一步失败就停在那里，不要继续：

1. **备份**（在已 root 的系统或 fastboot 里）
   ```sh
   dd if=/dev/block/by-name/recovery_a of=/sdcard/recovery_a_stock.img
   dd if=/dev/block/by-name/recovery_b of=/sdcard/recovery_b_stock.img
   dd if=/dev/block/by-name/super        of=/sdcard/super_stock.img   # 扩容前必须
   ```
2. **刷入** `recovery_a` / `recovery_b`，`fastboot reboot recovery`。
3. **解密**：进 recovery，先跑一次自检
   ```sh
   adb shell skyward check
   ```
   它把槽位、底包、已装系统、`/data` 表项、密钥文件大小、密钥链服务是否注册、
   更新状态，以及每个服务自己错误的最后几行一次打印出来。**这份输出就是排查时最该贴的。**
   要更全的材料，在电脑上跑 `tools/collect-report.sh`，它会连 `dmesg` 和
   `recovery.log` 一起收进一个文件。
   然后看 `/skyward/run/recovery.log`
   - 期望 `keymint` / `gatekeeper` 注册，`/dev/block/dm-* /data f2fs rw`
   - 输锁屏密码后 `/sdcard` 可读
   - 服务起不来时看 `/skyward/run/services/<程序名>.log`：
     `CANNOT LINK EXECUTABLE` 是缺库，`the service refused the call` 是安全元件/weaver，
     两者处理方式不同
   - 也可以直接跑一次确认库路径：
     ```sh
     LD_LIBRARY_PATH=/system/lib64:/vendor/lib64:/vendor/lib64/hw:/mnt/system/system/lib64 \
       timeout 3 /vendor/bin/hw/android.hardware.security.keymint-service-qti
     ```
4. **完整 OTA**：装一个 OS4.0.4.0 完整包，确认能写、能切槽、能起机。
5. **逻辑分区扩容**（可选，风险最高）：
   - 先准备一个比现有 `system` 大的逻辑分区镜像
   - 记录 `lpdump` 前后 result，确认只增加了本分区的 extent，另一槽未变
   - 刷完重启，确认能挂载
6. **sparse super**：准备原厂 `super.img`（sparse）刷入，确认重启后两槽分区都在。

第 5、6 步一旦中途断电，按厂商方式重刷完整 `super` 恢复；提交元数据本身不是掉电原子操作。
