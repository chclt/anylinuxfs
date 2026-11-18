# MTP 方案与分支索引

归档日期：2026-11-17。此索引用于区分实验结果与已接入的功能，不代表所有方案均可直接部署。

| 分支 | 方案 | 基线与状态 |
|---|---|---|
| `codex/mtp` | jmtpfs/libmtp + Linux VM + NFS | `2f1d768` 的运行代码，补充历史实验和文档 |
| `codex/archive/mtp-writeback` | 自研持久化写回 + VM + NFS | 保留 `457d96a`，后台写回与恢复路线归档 |
| `codex/archive/mtp-direct` | 自研直接写入 + VM + NFS | 保留 `d7b2c49`，直接写入与请求调度路线归档 |
| `codex/archive/mtp-native-writeback` | 原生 libmtp + 自研缓存/写回 + macFUSE FSKit | 基于 main 保存隔离实验；也保存未运行成功的直接 FSKit 对照 |
| `codex/archive/mtp-file-provider` | File Provider | 基于 main 保存模拟后端原型；未接真实 MTP |
| `codex/archive/mtp-aft-native` | AFT + 原生 macFUSE FSKit | 基于 main 保存读回复与 AppleDouble 补丁、实机和平台对照 |
| `codex/mtp-aft` | AFT + Linux VM + 标准 NFS | 从 main `9602097` 新建；已接入 CLI 与构建；验证范围见 `docs/mtp.md` |

`codex/mtp-architecture` 原有分支和 Disk Arbitration 生命周期修复的 stash 保留不动。
各分支的 `research/mtp/<方案>/README.md` 说明源代码入口、恢复方式与验证边界。
归档没有推送远端；`origin/*` 只是整理时的本地远端跟踪引用，本次未 fetch。

## 保存边界

- 已提交运行代码通过原提交祖先关系保存；原型源码、补丁、实验脚本、结果 JSON 另行归档。
- `manifest.json` 记录复制文件的原位置、大小和 SHA-256；`files/` 保留原相对路径。
- 二进制、VM/rootfs 镜像、视频及随机测试数据、缓存、任务数据库、抓包和完整运行日志不进 Git。
- 原始 `tmp/` 文件未删除，仍可用于复核。归档是研究记录，不是可直接运行的自动化产品。
- 脚本可能使用原机器绝对路径、旧对象 ID 和测试目录；运行前必须检查并适配。
- 分支切换不会替换被忽略的 `bin/`、`libexec/` 或已安装的 guest helper；测试某分支前必须重新构建并确认 guest helper 的来源。

## 当前取舍

推进 AFT + VM + NFS，保留 File Provider 候选；原生 AFT + macFUSE FSKit 暂停。
无时区的 MTP 时间按电脑时区解释；不改变手机设置。不能保留源修改时间、创建日期缺失暂作为限制。
最新 AFT 刷新补丁主要在三星 Z Fold 6 验证，不能套用旧驱动在小米、鸿蒙上的结论。
