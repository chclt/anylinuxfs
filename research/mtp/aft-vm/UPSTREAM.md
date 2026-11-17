# 上游重建基线

仓库：https://github.com/whoozle/android-file-transfer-linux

提交：`712dce6ea71793f6f8454e8ce75a0fd4ea553226`（v4.6）。

源码 URL：`https://codeload.github.com/whoozle/android-file-transfer-linux/tar.gz/712dce6ea71793f6f8454e8ce75a0fd4ea553226`

压缩包 SHA-256：`03ba4b3bc1e31e5656fdf63b213ca259323c0fbb4b187843ce19209338ae63fa`。

校验并解压原版源码后，在源码根目录使用 `git apply /绝对路径/aft-*-complete.patch`（替换为本方案确切文件名）。
完整补丁包含各阶段修改，不重复应用实验目录中的增量补丁。`AFT-LICENSE` 来自上游。
