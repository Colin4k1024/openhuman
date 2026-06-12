# OpenHuman 开发模式 (Development Model)

## 1. 整体开发模式

**模式**: Fork + PR 协作模式 (GitHub Flow 变体)

```
开发者 Fork → Feature Branch → 本地开发 → CI 验证 → PR Review → Merge to main
```

- 所有贡献通过个人 Fork 提交，不直接推送 upstream
- 单一主干分支 `main`，无 develop / release 分支
- 每个 PR 必须通过自动化质量门禁才能合并

---

## 2. 分支策略

| 分支类型 | 命名规范 | 说明 |
|----------|----------|------|
| `main` | — | 唯一受保护分支，始终可发布 |
| Feature | `feat/<描述>` | 新功能开发 |
| Fix | `fix/<描述>` | Bug 修复 |
| Refactor | `refactor/<描述>` | 重构 |
| Docs | `docs/<描述>` | 文档变更 |

**Commit 规范**: Conventional Commits

```
<type>(<scope>): <description>

类型: feat, fix, refactor, docs, test, chore, perf, ci
范围: agent, memory, channels, meet, ui, core, observability...
```

---

## 3. CI/CD Pipeline

### 质量门禁 (PR 合并前)

| 门禁 | 工具 | 阈值/规则 |
|------|------|-----------|
| 代码覆盖率 | diff-cover (Vitest + cargo-llvm-cov) | **变更行 ≥ 80%** |
| 类型检查 | `tsc --noEmit` | 零错误 |
| Lint | ESLint + Prettier | 零 warning |
| Rust 检查 | `cargo fmt --check` + `cargo check` | 零错误 |
| PR Checklist | `scripts/check-pr-checklist.mjs` | 必填项完整 |
| Coverage Matrix | 特性覆盖矩阵同步 | 新增功能需更新 |
| CEF Pin Guard | vendored CEF 版本锁定检查 | 防止意外升级 |

### 构建流水线

| Workflow | 触发条件 | 产出 |
|----------|----------|------|
| `build-desktop.yml` | PR / push to main | Win/Mac/Linux 桌面包 |
| `build-windows.yml` | Windows 专项构建 | .msi / .exe |
| `ios-compile.yml` | iOS 代码编译验证 | 编译通过 |
| `android-compile.yml` | Android 代码编译验证 | 编译通过 |
| `e2e.yml` | PR | Tauri E2E 测试 |
| `coverage.yml` | PR | 覆盖率报告 + 门禁 |

### 发布流水线

| Workflow | 说明 |
|----------|------|
| `release-staging.yml` | 预发布环境 |
| `release-production.yml` | 正式发布 |
| `release-packages.yml` | npm/cargo 包发布 |
| `deploy-smoke.yml` | 发布后 Smoke 验证 |
| `installer-smoke.yml` | 安装包完整性验证 |

---

## 4. 测试策略

### 分层测试

| 层级 | 工具 | 范围 | 目标 |
|------|------|------|------|
| 单元测试 (前端) | Vitest | 组件、hooks、utils | 行为验证 |
| 单元测试 (Rust) | cargo test | 领域逻辑、RPC handler | 正确性 |
| 集成测试 | Mock Backend + JSON-RPC E2E | Core ↔ 前端 RPC 链路 | 端到端正确 |
| E2E 测试 | WDIO + Appium (macOS) / tauri-driver (Linux) | 关键用户流程 | 回归保护 |

### Mock 策略

- 共享 Mock Backend (`scripts/mock-api-server.mjs`)
- 支持行为注入 (`POST /__admin/behavior`)
- Rust 测试通过 `scripts/test-rust-with-mock.sh` 启动

### 调试工具

```bash
pnpm debug unit [file] [-t "test name"]   # Vitest 单元
pnpm debug e2e <spec>                      # WDIO E2E
pnpm debug rust [filter]                   # cargo test
pnpm debug logs [last|prefix]              # 查看日志
```

---

## 5. 技术栈版本管理

| 依赖 | 版本锁定方式 |
|------|------------|
| Rust | `rust-toolchain.toml` (1.93.0) |
| Node.js | `package.json` engines (≥24.0.0) |
| pnpm | `packageManager` 字段 (10.10.0) |
| Tauri CEF CLI | vendored submodule + pin guard CI |
| 依赖锁 | `pnpm-lock.yaml` + `Cargo.lock` |

---

## 6. 代码组织原则

### Rust Core

- **Controller Registry Pattern**: 每个领域通过 `schemas.rs` 注册 RPC handler
- **EventBus**: 跨领域通信走 pub/sub，不直接耦合
- **模块隔离**: 新功能必须在 `src/openhuman/<domain>/` 下建独立子目录
- **RpcOutcome<T>**: 统一 RPC 返回值契约

### 前端

- **Provider Chain**: 固定嵌套顺序 (ErrorBoundary → Redux → CoreState → Socket → ChatRuntime)
- **i18n 全覆盖**: 所有 UI 文本走 `useT()`，12 语言同步
- **静态 import only**: 生产代码禁止动态 `import()`
- **Config 集中**: `VITE_*` 统一通过 `utils/config.ts` 导出

### Tauri Shell

- **不注入 JS**: CEF Webview 禁止新增 JavaScript 注入
- **Core in-process**: 无 sidecar，tokio task 内嵌运行
- **Bearer Auth**: 每次启动生成随机 token，内存传递

---

## 7. 安全模型

| 层级 | 机制 |
|------|------|
| RPC 认证 | Per-launch Bearer Token (内存生成，不写 env) |
| 命令分类 | `classify_command` → Read/Write/Network/Install/Destructive |
| 权限分级 | readonly / supervised (ask-before-edit) / full |
| 路径保护 | `is_always_forbidden` 系统目录无条件阻断 |
| 审批门控 | ApprovalGate (UI 弹窗确认高风险操作) |
| 凭证存储 | OS Keyring + Vault，不进 Redux |
| 传输加密 | iOS ↔ Desktop XChaCha20-Poly1305 E2E |

---

## 8. 发布模式

- **桌面端**: Win (.msi/.exe) + macOS (.dmg) + Linux (.AppImage/.deb)
- **自动更新**: Tauri updater 机制
- **环境分离**: staging / production 独立流水线
- **Feature Flag**: 通过 config `[autonomy]` 块控制功能开关
- **发布后验证**: `deploy-smoke.yml` + `installer-smoke.yml`

---

## 9. 协作工具链

| 用途 | 工具 |
|------|------|
| 项目管理 | GitHub Issues + Linear |
| 代码托管 | GitHub (tinyhumansai/openhuman) |
| CI/CD | GitHub Actions |
| 错误监控 | Sentry |
| 文档 | GitBook (`gitbooks/developing/`) |
| 依赖安全 | Dependabot / GitHub Advisory |
