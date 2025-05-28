# RustGit

这是一个用 Rust 语言实现的简化版 Git。

## 功能

本项目旨在重新实现 Git 的一些核心功能，让你深入了解 Git 的内部工作原理。

目前已实现以下命令：

*   `init`：初始化一个新的 Git 仓库。
*   `add`：将文件内容添加到索引。
*   `commit`：记录对仓库的更改。
*   `branch`：列出、创建或删除分支。
*   `checkout`：切换分支或恢复工作树文件。
*   `merge`：合并两个或多个开发历史。
*   `pull`：从远程仓库获取并集成。
*   `push`：更新远程引用以及关联的对象。
*   `fetch`：从另一个存储库下载对象和引用。
*   `cat-file`：提供仓库对象的内容或类型和大小信息。
*   `rm`：从工作区和索引中删除文件。

## 模块

项目主要包含以下模块：

*   `commands`：实现了各个 Git 命令的逻辑。
*   `repository`：处理 Git 仓库的内部结构，包括对象（objects）、引用（refs）和索引（index）。

## 如何使用

### 编译运行
cargo build --release

如果是需要提交到oj的rust-git可执行文件,需要

"cargo build --features online_judge --release"

之后运行rust-git即可.

### 提交

运行pack.sh 即会自动生成一个符合提交格式的压缩包,自动使用 "cargo build --features online_judge --release" 编译

注意需要在x86 ISA下编译才行,macOS本地编译的结果提交后是无法运行的.


### Help

rust-git -h/help 即可
