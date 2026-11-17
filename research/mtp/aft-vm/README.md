# AFT + Linux VM + 标准 NFS：接入准备

本分支从 main `9602097` 新建。**目前只保存接入材料，AFT 尚未接入正式运行代码或构建。**
没有继承自研持久化写回、直接写入文件系统或 NFS 中继。

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
这些环境设置尚需正式接入，补丁本身不会自动把 Mac 时区带进 guest。

已接受限制：手机拒绝访问的目录仍报错；源修改时间可能无法写回；创建日期缺失暂不修。
上游跨目录 rename 返回 EXDEV，不应宣传完整 POSIX 文件系统语义。

## 下一步接入顺序

1. 从 jmtpfs 分支有选择地迁入 USB 转发和宿主/guest MTP 挂载生命周期；排除 jmtpfs 私有缓存和写回实现。
2. 固定 AFT 上游版本和校验值，接入 guest 构建与完整补丁，替换实验绝对路径。
3. 接入标准 NFS 配置和只读时区资源，确保普通磁盘挂载行为不变。
4. Review 补丁职责，验证手机端已有文件内容变化的刷新，以及刷新补丁下的大视频响应。
5. 构建并完成三星回归，再用小米/鸿蒙验证；旧驱动的实机结果不能代替 AFT 回归。

实验脚本可能依赖旧分支的 CLI，当前 main 基线不能直接运行它们；正式功能接入后再执行实机回归。

## 如何读取与恢复归档

`files/tmp/` 对应原工作区的 `tmp/`。先参考 `manifest.json`，只把需要的实验目录复制到**独立实验环境**的同名位置；不要覆盖现有快照和运行任务。
脚本保留历史原样，包含绝对路径和专用测试对象名称，不能不经检查直接对手机执行。
构建产物、视频、VM 镜像和已下载的工具包需重新准备。所有归档文件已与原文件逐一校验 SHA-256。
本次仅整理 Git 和研究记录，没有重新运行实机实验，不把历史通过结果表述为本次回归。
