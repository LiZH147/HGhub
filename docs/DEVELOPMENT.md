# 客户端开发指南

[返回项目首页](../README.md)

本文面向参与客户端开发的贡献者。插件作者请先阅读[插件开发教程](developers/index.md)。

## 技术与目录

| 层 | 技术 |
| --- | --- |
| 应用框架 | Rust、Tauri 2 |
| 界面 | React、TypeScript、Vite、Tailwind CSS |
| 播放组件 | hls.js |
| 网络 | reqwest、rustls |

```text
src/                    界面、交互、状态管理与组件
src-tauri/              原生能力与 Rust 实现
scripts/                开发、构建与验证脚本
tests/                  本地测试页面与回归用例
docs/                   技术说明与维护记录
```

## 本地开发

基础环境：Node.js 22、pnpm 10、Rust 1.90 或更高版本，以及目标平台所需的 Tauri 开发环境。

```bash
pnpm install --frozen-lockfile
pnpm dev             # 前端开发
pnpm app:dev         # 桌面应用开发
pnpm build           # 类型检查与前端构建
pnpm app:build       # 桌面应用构建与打包
```

各平台配置位于 `src-tauri/tauri*.conf.json`。移动端及 macOS 开发需要相应平台的 SDK 和工具链。

Android 构建另需 JDK 17、Android SDK / NDK 和 libclang。设置 `JAVA_HOME`、`ANDROID_HOME`、`NDK_HOME`，并让 `LIBCLANG_PATH` 指向 libclang 动态库所在目录；插件引擎会使用 NDK 头文件生成 Android 绑定。首次运行 `pnpm tauri android init --ci`，随后可用 `pnpm tauri android build --debug --target x86_64 --apk --ci` 构建模拟器安装包（ARM64 设备将目标改为 `aarch64`）。

- exFAT 等不支持符号链接的文件系统需保留 `.npmrc` 中的 `node-linker=hoisted`；部分原生构建步骤仍可能受文件系统限制。
- 依赖安装脚本的允许列表见 `package.json` 与 `pnpm-workspace.yaml`。
- 验证命令及当前支持范围见[实现进度](plugin-protocol/IMPLEMENTATION.md)。

## Android 出包（GitHub Actions）

本机不装 Android SDK 也能出包：推送分支后在 Actions 里选 **Android APK** → Run workflow。

- `Use workflow from` 决定打包的代码分支；也可以用 `ref` 输入填分支名、tag 或 commit。
- `target` 选 CPU 架构（手机一般是 aarch64），`build_type` 选 debug 或 release。
- 产物在本次 run 的 Artifacts 里，是含 APK 的 zip。

debug 包用 runner 生成的调试密钥签名，不同次构建的密钥不同，覆盖安装会失败。需要可升级的正式包时用 release，并在仓库 Secrets 里配置四个值：

```powershell
# 生成一次并妥善备份：丢失后无法用同一签名继续升级
keytool -genkeypair -v -keystore hghub-release.jks -alias hghub -keyalg RSA -keysize 2048 -validity 10000
certutil -encode hghub-release.jks keystore.b64   # 去掉首尾两行后再填入 Secret
```

| Secret | 内容 |
| --- | --- |
| `ANDROID_KEYSTORE_BASE64` | keystore 文件的 base64 |
| `ANDROID_KEY_ALIAS` | 上面的 `-alias` 值 |
| `ANDROID_KEY_PASSWORD` | 该别名的口令 |
| `ANDROID_STORE_PASSWORD` | keystore 口令 |

工作流会解码 keystore 并用 `scripts/android-signing.py` 给生成的 gradle 工程注入 `signingConfigs`（`src-tauri/gen/android` 每次重新生成，所以不提交签名配置）。

两点注意：release 沿用 Tauri 模板的 R8 压缩配置，首次上线前要在真机上验证；`versionCode` 由 `package.json` 版本推导，升级安装前先提版本号。


## 开发者文档站

```bash
pnpm docs:build
pnpm docs:test
pnpm docs:preview
```

默认预览地址为 `http://127.0.0.1:4174/HGhub/`。静态页面、示例与公开 Agent 技能包输出到 `dist-docs/`。
Pages 配置与部署步骤见[文档站部署说明](developers/deployment.md)。

## 进一步阅读

- [插件开发教程](developers/index.md)：编写内容源或解析器插件。
- [插件协议](plugin-protocol/README.md)：接口与数据约定。
- [当前支持范围](developers/status.md)：实现状态与平台验证边界。
