# 记忆系统长期计划拆分

> 将路线图中的 3 项长期目标拆解为可执行的子阶段。
> 最后更新：2026-06-11

---

## 7. 分布式记忆（多设备同步）

### 目标

让用户在多台设备（桌面 + 手机 + 服务端）共享同一份记忆，支持离线写入 + 后续合并。

### 阶段拆分

#### 7.1 数据模型 + 变更日志（基础）

- [ ] 为每条记忆操作（insert/update/delete）生成带 Lamport 时间戳的 `ChangeEntry`
- [ ] 在 `chunks.db` 新增 `mem_change_log` 表：`(entry_id, lamport_ts, device_id, op_type, payload_json, created_at)`
- [ ] 每次 `upsert_document` / `kv_set` / `delete` 时同步写入 change log
- [ ] 实现 `get_changes_since(lamport_ts) -> Vec<ChangeEntry>` 查询

**产出**：`crates/openhuman-memory/src/sync_protocol/changelog.rs`

#### 7.2 设备身份 + 发现

- [ ] 生成 per-device X25519 密钥对，存储在 workspace `device.key`
- [ ] 定义 `DeviceIdentity { device_id, public_key, display_name, last_seen }`
- [ ] LAN 发现：mDNS 广播 `_openhuman-memory._tcp`，携带 device_id + port
- [ ] Cloud relay 注册：POST device identity to backend relay endpoint

**产出**：`crates/openhuman-memory/src/sync_protocol/device.rs`

#### 7.3 同步传输层

- [ ] 定义 `SyncTransport` trait：`push_changes(entries) / pull_changes(since_ts) -> Vec<ChangeEntry>`
- [ ] `LanTransport`：HTTP POST/GET over LAN (mDNS discovered peer)
- [ ] `RelayTransport`：通过 backend socket.io relay 中转（E2E encrypted）
- [ ] `DirectTransport`：两台设备直连（同网段 TCP）

**产出**：`crates/openhuman-memory/src/sync_protocol/transport.rs`

#### 7.4 冲突解决（CRDT / LWW）

- [ ] KV 存储：Last-Writer-Wins (LWW)，以 Lamport 时间戳 + device_id 字典序 tie-break
- [ ] Document 存储：Merge（同 namespace+key 的内容取最新 updated_at）
- [ ] Chunk 存储：idempotent by id（id 相同 = 同一块，直接跳过）
- [ ] Entity 存储：union merge（同 entity 不同 handles 合并）
- [ ] 冲突日志：无法自动解决的冲突存入 `mem_conflicts` 表，等用户裁定

**产出**：`crates/openhuman-memory/src/sync_protocol/merge.rs`

#### 7.5 端到端加密

- [ ] 复用现有 `XChaCha20-Poly1305 over X25519` 管线（tunnel crypto）
- [ ] 每次同步建立临时会话密钥（X25519 ephemeral）
- [ ] 变更内容全程加密传输，relay 不可读

**产出**：`crates/openhuman-memory/src/sync_protocol/crypto.rs`

#### 7.6 集成 + E2E 测试

- [ ] 两个内存实例互相同步的集成测试
- [ ] 模拟网络分区 + 重连后合并
- [ ] 性能基准：1000 条变更同步延迟 < 2s

---

## 8. 记忆图谱（知识图谱 + 图嵌入）

### 目标

从记忆中自动构建实体关系图，支持图查询（"谁和谁合作过"、"哪些项目相关"）和图嵌入向量检索。

### 阶段拆分

#### 8.1 持久化图结构

- [ ] 新增 `mem_graph_nodes` 表：`(node_id, entity_type, label, properties_json, created_at, updated_at)`
- [ ] 新增 `mem_graph_edges` 表：`(edge_id, source_id, target_id, relation_type, weight, evidence_json, first_seen, last_seen)`
- [ ] 从现有 `entities` 模块的 entity index 自动导入初始节点
- [ ] 实现 CRUD API：`add_node`, `add_edge`, `remove_edge`, `update_weight`

**产出**：`crates/openhuman-memory/src/graph/persistent_store.rs`

#### 8.2 自动关系发现

- [ ] 共现关系：同一 chunk 中出现的两个实体自动建立 `co_occurs_with` 边
- [ ] 时序关系：同一 session 中先后提到的实体建立 `follows` 边
- [ ] 主谓宾提取（简单规则）：`<Person> works at <Organization>` → `employed_by` 边
- [ ] LLM 辅助关系提取（可选，需 ChatProvider）

**产出**：`crates/openhuman-memory/src/graph/discovery.rs`

#### 8.3 图查询 API

- [ ] `neighbors(node_id, max_hops, relation_filter) -> SubGraph`
- [ ] `shortest_path(source, target) -> Vec<Edge>`
- [ ] `common_connections(node_a, node_b) -> Vec<Node>`
- [ ] `subgraph_by_type(entity_type, limit) -> SubGraph`
- [ ] 通过 memory-server RPC 暴露：`memory.graph_neighbors`, `memory.graph_path`

**产出**：`crates/openhuman-memory/src/graph/query.rs`

#### 8.4 图嵌入

- [ ] 实现 TransE 或 Node2Vec 的轻量 Rust 版本
- [ ] 每个节点生成固定维度的图嵌入向量
- [ ] 存入 `mem_graph_node_embeddings` 表
- [ ] 支持"给定节点，找语义最近的 N 个节点"查询
- [ ] 定期重算（增量或全量）

**产出**：`crates/openhuman-memory/src/graph/embedding.rs`

#### 8.5 时序图（关系演化）

- [ ] 边携带 `first_seen` / `last_seen` / `interaction_count`
- [ ] 支持时间窗口查询："过去 30 天最活跃的关系"
- [ ] 关系衰减：长期未见的边权重自动降低
- [ ] 可视化接口：导出为 DOT/JSON 格式

**产出**：`crates/openhuman-memory/src/graph/temporal.rs`

---

## 9. 联邦学习（跨用户模式发现）

### 目标

在保护隐私的前提下，发现跨用户的记忆模式（"大多数用户在做 X 后会做 Y"），用于预测和推荐。

### 阶段拆分

#### 9.1 本地模式提取

- [ ] 定义 `Pattern` 类型：`(trigger_context, action_taken, frequency, confidence)`
- [ ] 从用户本地记忆中提取行为模式：
  - 时序模式：A 事件后通常跟 B 事件
  - 偏好模式：用户在 X 场景下偏好 Y 方式
  - 知识模式：用户对某领域的理解深度
- [ ] 模式置信度评分（出现频率 × 一致性）
- [ ] 存入本地 `mem_patterns` 表

**产出**：`crates/openhuman-memory/src/federation/local_patterns.rs`

#### 9.2 差分隐私

- [ ] 实现 ε-差分隐私机制：对模式频率添加 Laplace 噪声
- [ ] 隐私预算管理：每轮上报消耗 ε，累计不超过阈值
- [ ] 敏感信息过滤：模式中不包含具体人名、组织名、数字等 PII
- [ ] 泛化处理：`"张三" → "Person"`, `"字节跳动" → "TechCompany"`

**产出**：`crates/openhuman-memory/src/federation/privacy.rs`

#### 9.3 安全聚合协议

- [ ] 实现 Secure Aggregation（加法秘密共享）
- [ ] 每个客户端对本地模式向量加密 → 发送到聚合服务器
- [ ] 服务器只能看到聚合结果，无法还原单个用户的模式
- [ ] 最少 K 个参与者才能解密聚合结果

**产出**：`crates/openhuman-memory/src/federation/aggregation.rs`

#### 9.4 群体模式服务

- [ ] 聚合服务端：收集各客户端的加密模式 → 解密聚合 → 生成群体模式
- [ ] 群体模式下发：新用户可获得"大多数人的常见操作序列"
- [ ] 冷启动加速：基于群体模式为新用户生成初始记忆框架
- [ ] 分群：不同用户群体（开发者 / 设计师 / 管理者）的模式分开聚合

**产出**：`crates/openhuman-memory-federation-server/`（新 crate）

#### 9.5 隐私审计 + 合规

- [ ] 用户可查看自己贡献了哪些模式（脱敏后）
- [ ] 一键退出：删除该用户在聚合池中的贡献
- [ ] 合规报告：记录每次上报的隐私预算消耗
- [ ] GDPR Right to Erasure 支持

**产出**：`crates/openhuman-memory/src/federation/audit.rs`

---

## 依赖关系

```
7.1 数据模型 ──→ 7.2 设备发现 ──→ 7.3 传输层 ──→ 7.4 冲突解决 ──→ 7.5 加密 ──→ 7.6 E2E测试
                                                        ↓
8.1 持久化图 ──→ 8.2 关系发现 ──→ 8.3 图查询 ──→ 8.4 图嵌入 ──→ 8.5 时序图
                                                        ↓
9.1 本地模式 ──→ 9.2 差分隐私 ──→ 9.3 安全聚合 ──→ 9.4 群体服务 ──→ 9.5 审计
```

**独立性**：7/8/9 三条线互不阻塞，可并行推进。8.2 的"共现关系"是 graph 最有价值的快赢。

## 建议执行优先级

1. **8.1 + 8.2**（图谱持久化 + 自动关系发现）— 当前 `graph` 模块已 un-gate，基础最好
2. **7.1 + 7.2**（变更日志 + 设备身份）— 为同步做数据准备
3. **8.3**（图查询 API）— 最直接的用户价值
4. **7.3 + 7.4**（传输 + 合并）— 核心同步逻辑
5. **9.1 + 9.2**（本地模式 + 隐私）— 联邦学习基础
6. 其余按需推进

---

## 技术选型备忘

| 维度 | 选择 | 备选 | 原因 |
|------|------|------|------|
| CRDT 库 | 自研 LWW | `yrs` (Yjs Rust) | 记忆数据模型简单，不需通用 CRDT |
| 图存储 | SQLite 邻接表 | `petgraph` in-memory | 需持久化，SQLite 已在栈中 |
| 图嵌入 | 自研 TransE | `dgl` via Python | Rust 原生，无 Python 依赖 |
| 差分隐私 | 自研 Laplace | `opacus` (PyTorch) | 轻量级，不需 tensor framework |
| 安全聚合 | Shamir Secret Sharing | Google FLAME | 实现简单，满足需求 |
| 设备发现 | `mdns-sd` crate | `zeroconf` | 纯 Rust，跨平台 |
