# 记忆系统独立化路线图

> 基于 `memory-extraction-report.md` 的短中长期计划，结合当前实际进度更新。
> 最后更新：2026-06-10

---

## 当前状态总结

| 维度 | 状态 |
|------|------|
| 独立编译（standalone） | ✅ 0 errors, 0 warnings |
| `__full_app` 编译 | ✅ 0 errors |
| 工作空间集成 | ✅ root `cargo check` 通过 |
| 测试 | ✅ 539 tests（534 lib + 5 integration） |
| API 审查 | ✅ 5 项 HIGH/MEDIUM 问题已修复 |
| Un-gated 模块 | `entities`, `graph` |
| 仍 gated 模块 | `archivist`, `conversations`, `orchestration`, `sources`, `sync`, `tools_impl`, `learning_full` |

---

## 短期计划（下一迭代）

### ✅ 已完成

- [x] 完成 `__full_app` 编译（从 ~349 errors 降到 0）
- [x] 功能化 `embedding_ext`（Ollama/OpenAI/Cohere/Voyage 真实接入）
- [x] 集成测试 + examples
- [x] API 审查修复（CloudEmbedding Result 化、Config 单例收敛、字段门控、文件提取）

### ❌ 已放弃：Re-export + 删除原始代码

> **2026-06-10 实验结论**：尝试将 `memory_store` 替换为 `pub use openhuman_memory::store::*` 后发现 **157 个类型不匹配错误**。

**根本原因**：
1. **Rust 类型身份**：`main_crate::Chunk` ≠ `openhuman_memory::Chunk`，即使结构相同。11 个 memory 模块（~90K 行）必须作为一个原子操作全部切换，不可逐模块迁移。
2. **可见性差距**：extracted crate 有 12+ `pub(crate)` 函数被主 crate 跨模块访问，需全部提升。
3. **`impl` 不可跨 crate**：主 crate 对 extracted crate 类型不能定义新方法。

**决策**：保持双份代码。Extracted crate 作为独立产品面向外部消费者；主 crate 保留自己的实现供桌面应用使用。两者会自然 diverge（主 crate 做 product feature，extracted crate 做 library API），这是合理的分叉。

### 🔲 替代路径（中期可选）

- [ ] **共享类型层统一**
  - 已有 `openhuman-memory-types` 做类型共享（`Chunk`, `Metadata`, `SourceKind`）
  - 可继续扩展，让更多核心类型从 types crate 统一导入
  - 这样两边的 `Chunk` 是同一个类型，为未来合并铺路

- [ ] **解耦更多 gated 模块**
  - `archivist`：依赖 `tree::tree` + `tree::io`，需先 un-gate tree runtime
  - `conversations`：依赖 event bus，需将 event traits 提升到核心层
  - `orchestration`：依赖 `store::MemoryClient` + `sync`，最后处理

---

## 中期计划

### 4. 独立 binary `openhuman-memory-server`

- HTTP API 暴露记忆能力（Axum）
- 端口：`/rpc`（JSON-RPC 兼容主应用协议）
- 路由：`memory.upsert_document`, `memory.search`, `memory.kv_*`, `memory.score_chunk`
- 认证：Bearer token
- 部署：Docker 镜像，单文件 binary
- 依赖：`openhuman-memory` (standalone, no `__full_app`)

### 5. Python bindings (PyO3)

- Crate: `openhuman-memory-python`
- 暴露：`MemoryStore`, `UnifiedMemory`, `score_chunk`, `ScoringConfig`
- 分发：`maturin` build → PyPI wheel
- 目标用户：Python AI Agent 框架（LangChain, AutoGen, CrewAI）

### 6. WASM 编译

- 后端替换：SQLite → IndexedDB (via `web-sys`)
- 或：SQLite WASM (`sql.js` / `wa-sqlite`)
- 用途：浏览器端本地记忆（PWA、Chrome Extension）
- Feature flag: `wasm` 与 `native` 互斥

---

## 长期计划

### 7. 分布式记忆（多设备同步）

- 协议：CRDT 或向量时钟
- 传输：端到端加密（XChaCha20-Poly1305，复用现有 tunnel 加密）
- 冲突解决：last-writer-wins for KV，merge for documents/chunks
- 设备发现：LAN mDNS + cloud relay fallback

### 8. 记忆图谱

- 当前 `graph` 模块：只读查询层，从 entity index 派生边
- 目标：
  - 持久化图结构（专用 graph storage，非 SQLite 行式）
  - 图嵌入（TransE / GraphSAGE）
  - 关系推理查询（"谁认识谁"、"哪些项目相关"）
  - 时序图（关系随时间演化）

### 9. 联邦学习

- 目标：跨用户记忆模式发现（隐私保护）
- 方法：差分隐私 + 安全聚合
- 用例：群体记忆模式（"大多数用户在 X 之后会做 Y"）
- 前提：分布式记忆基础设施就绪

---

## 技术债务 & 已知限制

| 项目 | 说明 | 优先级 |
|------|------|--------|
| `reqwest` + `rustls-tls` 过重 | 对只用 Ollama 的消费者编译时间长 | MEDIUM |
| inline test 中有 43 个 `__full_app` 依赖 | 已 gate，不影响 standalone，但限制 `cargo test --features __full_app` | LOW |
| README 示例中部分 API 为示意性 | `search_fts`/`with_connection` 签名可能与实际不完全一致 | LOW |
| bridge trait 中 `ChatProvider::complete_chat` 签名 | 需随主应用 inference 重构同步 | MEDIUM |
| `learning` standalone stub 功能有限 | 只有空壳，无真正的学习反思 | LOW (intentional) |

---

## 验证命令

```bash
# Standalone
cd crates/openhuman-memory && cargo check && cargo test

# Full app
cargo check --features __full_app

# Workspace
cd /path/to/openhuman && cargo check

# Integration tests only
cargo test -p openhuman-memory --test standalone_store --test tree_scoring
```

---

## 相关文件

| 文件 | 用途 |
|------|------|
| `crates/openhuman-memory/README.md` | 使用文档 |
| `crates/openhuman-memory/Cargo.toml` | 依赖与 feature 定义 |
| `crates/openhuman-memory/src/lib.rs` | 模块入口 + 门控 |
| `crates/openhuman-memory/src/bridge/` | 外部依赖抽象层 |
| `crates/openhuman-memory/src/embedding_ext.rs` | 嵌入 provider 工厂 |
| `crates/openhuman-memory/src/config.rs` | 独立配置 |
| `crates/openhuman-memory/tests/` | 集成测试 |
| `crates/openhuman-memory/examples/` | 使用示例 |
| `docs/architecture/memory-extraction-report.md` | 原始方案汇报 |
