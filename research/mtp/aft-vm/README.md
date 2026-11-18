# AFT + Linux VM + 标准 NFS：接入准备

本分支从 main `9602097` 新建。已开始接入正式 CLI、USB 转发、guest 构建和 NFS 配置；当前用法见 [docs/mtp.md](../../../docs/mtp.md)。本目录仍保留原始实验快照。
没有继承自研持久化写回、直接写入文件系统或 NFS 中继。

## 鸿蒙 6.1 只读接入（2026-11-18）

逐项对照确认了属性占位值、重命名等待事件、CloseSession 不回应，以及缺少 Android 编辑操作。目录、事件接收和退出清理已整理为 `scripts/aft-harmony.patch`；华为只读元数据兼容不关闭其他设备的属性批量查询。

实验版连续五次改名均在 7–13 毫秒完成，测试对象已删除。缩短 CloseSession 等待后，正常卸载约 1.5 秒，无需强杀，并通过不切模式直接重挂和新会话哈希校验。标准 1 MiB 整文件上传也通过逐字节读回，但用户选择先只读支持，不增加整文件暂存写回。

最终版本在挂载前拒绝不支持的 `-o rw`，随后 `-o ro` 重试、目录浏览、读取、EROFS 和退出检查均通过。小米、鸿蒙 7、鸿蒙拔线和 Finder UI 尚未用最终补丁回归。详见 [结构化结果](harmony61-result-20261118.json)。

同日三星 Z Fold 6 使用同一 helper 完成读写、USB 边界、覆盖截断、文件和目录改名，以及直接重挂和拔线重连后的 11 文件哈希校验。正常卸载约 0.58/0.57 秒；只读循环两次读取之间拔线后，下次打开返回 EIO，卸载约 0.36 秒，helper/VM 均退出 0。新写入和清理通过。另有未定位的非测试目录 InvalidObjectHandle 日志；本轮未做 Finder UI 或写事务中途拔线。详见 [三星回归记录](samsung-harmony-regression-20261118.json)。

上述实机结果对应 review 前 helper `9ee20aae…`。后续 review 修复移除了父目录缓存，改用当前 ObjectInfo 查询，并在返回尚未枚举的新父目录前补齐同一存储的归属；USB 取消改为只回收完成记录，事件处理与重新提交留给正常传输循环。修正版已构建，鸿蒙手机端移动目录的实机检查尚未进行；不把此前测试结果当作修正版覆盖。

## 已保存材料

- `aft-vm-complete.patch`：最终 `aft-time-fix` 源码相对 AFT v4.6 `712dce6ea71793f6f8454e8ce75a0fd4ea553226` 的完整补丁。
- 同名 `.patch.json`：改动文件的预期 SHA-256；已在原版源码上应用并逐文件验证。
- `files/tmp/mtp-upstream-evaluation/versions.json`：上游提交和源码压缩包校验值。
- `directory-fix/`、`exit-fix/`、`refresh-study/`、`mtime-study/`、`birthtime-study/`：各修复和未解决限制的实验记录。
- `files/tmp/mtp-upstream-evaluation/image/rootfs/opt/upstream/`：从实验 VM 提取的构建脚本、guest 启动脚本和协议探针源码。VM 镜像与构建产物不保存。
- 原版 jmtpfs、simple-mtpfs、go-mtpfs、AFT 的只读比较也保存在实验记录中，不把它们算作独立产品架构。

完整补丁包含目录查询成功后才发布缓存、断线/关闭错误处理、按需目录刷新、稳定 readdir cookie、FUSE export 支持和时间解析修复。
它已经包含分阶段补丁，不能叠加应用。上游许可证随附。

## 已验证配置与边界

候选配置：AFT FUSE -> Linux kernel NFS3；4 个 nfsd worker，max_block_size 1 MiB；宿主 sync、rwsize=1048576。
使用标准 NFS 挂载参数，不依赖自研请求中继，也不依赖临时修改宿主全局内核参数。
历史实验包含 Finder 大文件批量复制/浏览/覆盖/删除、哈希读回、写入中断及正常卸载。
后续小补丁验证了目录枚举失败不伪装为空目录、手机新增/改名/删除自动刷新、NFS 冷缓存句柄恢复。
无时区时间按电脑时区解释：实验使用只读复制的 Mac TZif 文件，并设置 AFT 进程 TZ；不改手机设置，也不改整个 VM 时区。
宿主接入代码另行提供只读时区资源；单独应用补丁不会自动把 Mac 时区带进 guest。

已接受限制：手机拒绝访问的目录仍报错；源修改时间可能无法写回；创建日期缺失暂不修。
上游跨目录 rename 返回 EXDEV，不应宣传完整 POSIX 文件系统语义。

### 结束编辑错误反馈：已接受，不修（2026-11-18）

FUSE `release` 中的 `EndEditObject` 失败可能无法传到 Finder。核对 AFT、[Android MTP 服务端](https://android.googlesource.com/platform/frameworks/av/+/refs/heads/main/media/mtp/MtpServer.cpp)及当前三星 Android 16 的系统 `libmtp.so` 后确认：正文在分块写入时直接写入目标文件，接收路径等待写入完成；`EndEditObject` 负责媒体信息重扫描和编辑资源释放，不是整文件数据提交。因此，所有写入成功、仅结束编辑失败时，不应将其表述为正文未完整复制；影响范围是媒体信息更新及编辑状态清理，结论不外推到未经检查的设备实现。

临时实验将结束编辑接入 `fsync`，确认每个 1 MiB NFS 写入都会触发。32 MiB 同步写入的单次配对测量由 1.737 秒（19.31 MB/s）变为 2.714 秒（12.36 MB/s）。正常 Finder 复制及冷缓存读回哈希通过；在真实结束编辑成功后注入一次错误，Finder 报 `-36`，直接重试成功。这只验证错误传播，不代表真实手机会话故障恢复。

用户决定保留此已知缺陷，不采用逐块结束编辑的修法。实验 helper 已撤下，测试文件已清理，原 helper 已恢复并重新挂载；正式实现不变。

## 正式接入状态

已接入 USB 转发、宿主/guest MTP 挂载生命周期、AFT guest 构建、NFS 配置及只读时区资源。
没有迁入 jmtpfs 私有缓存、自研写回队列或 NFS 中继。

本轮正式路径验证见 [integration-result-20261117.json](integration-result-20261117.json)：完整构建与 96 个 Rust 测试通过；三星普通用户读写、只读保护、正常卸载和不拔线重挂通过。
478 MB 视频写入约 37.8 秒，目录查询最大约 0.42 秒；新 VM 读回约 66.9 秒且 SHA-256 一致。测试目录已删除。

2026-11-18 追加接入 64 KiB USB 写请求和宿主零长度结束包修复。同一视频的完整 NFS 复制对照从 36.84 秒降至 23.94 秒；原版也存在的 65,524 字节边界超时在 USB 标志转换修复后消失。两种请求大小均通过七种文件大小的读回校验，详见 [MTP 验证记录](../../../docs/mtp.md#upload-and-usb-boundary-validation-2026-11-18)。

随后按日常 Android 文件内容传输的定位，接入 `aft-metadata.patch`：拒绝新建 `.DS_Store`、`._*` 及改名到这些名称，保留普通点文件和已有元数据文件。Finder 复制、覆盖和目录复制通过，正文哈希一致；标签与资源叉未保留且 Finder 没有额外提醒。这不是完整 Mac 文件备份语义，具体范围见 [元数据策略验证](../../../docs/mtp.md#finder-metadata-policy-validation-2026-11-18)。

仍需：Finder 手动交互和拔线回归、手机端已有文件内容变化刷新、小米/鸿蒙实机验证。旧驱动或旧实验结果不能代替这些检查。
历史实验脚本保留当时接口与路径；新版本回归使用当前 CLI。

## 如何读取与恢复归档

`files/tmp/` 对应原工作区的 `tmp/`。先参考 `manifest.json`，只把需要的实验目录复制到**独立实验环境**的同名位置；不要覆盖现有快照和运行任务。
脚本保留历史原样，包含绝对路径和专用测试对象名称，不能不经检查直接对手机执行。
构建产物、视频、VM 镜像和已下载的工具包需重新准备。所有归档文件已与原文件逐一校验 SHA-256。
归档时仅整理 Git 和研究记录；后续接入验证单独记录，不把历史通过结果表述为新版本回归。
