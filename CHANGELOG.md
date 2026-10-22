# 变更记录

本文件只记录**已发布到 crates.io 的版本**。版本号遵循
[语义化版本](https://semver.org/lang/zh-CN/)。

## 2.1.0 — 未发布

### 新增

- `ConversationUserSettings` 增加 `is_mention_only`（字段号 7）：
  只接收提到我的消息。与 `is_muted` 正交——muted 是「一条都别响」，
  mention_only 是「只有点名才响」，两者可同时开启。
- `ConversationParticipant` 增加 `visible_from_seq`（字段号 7）：
  该成员可见的历史下限。负值是「从加入时刻起」的哨兵，由核心解析成当时的 seq。

- `EventType` 增加 `EVENT_MESSAGE_PLUGIN_STATE = 17`，`Event.payload` 增加
  `PluginStateEvent plugin_state = 25`：插件实体的共享状态快照（`server_msg_id`、
  `plugin_id`、`entity`、`version`、`values_json`），按版本单调覆盖。

以上都是**向后兼容的新增**：字段号与枚举值此前未被占用，老客户端不发送、老服务端忽略。
因此升 minor 而非 major。对 `EventType` 或 `event::Payload` 做穷尽匹配的下游代码需要补上新分支。

## 2.0.1 — 2026-08-03

与实现层 1.1.0 对齐的契约层发布。
