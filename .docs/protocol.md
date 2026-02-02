# 资源岛游戏客户端网络协议通信手册
提示：为AI总结，不保证正确性，以实际为准。
## 概述

本手册面向客户端开发者，详细说明如何与资源岛游戏服务端进行WebSocket通信。

---

## 连接信息

### WebSocket连接地址
```
ws://{host}:{port}/ws/{player_name}
```

**参数说明：**
- `host`: 服务器地址（默认：127.0.0.1）
- `port`: 服务器端口（默认：8080）
- `player_name`: 玩家名称（URL路径参数）

**示例：**
```
ws://127.0.0.1:8080/ws/Alice
```

### 认证方式

服务器支持两种认证方式（如果启用了token认证）：

#### 1. Query参数认证
```
ws://127.0.0.1:8080/ws/Alice?token=your_token_here
```

#### 2. HTTP Header认证
```
Authorization: your_token_here
```

**注意：** 如果服务器配置无需认证，则无需认证。

---

## 消息格式

所有消息使用 **JSON格式**，采用 **tagged union** 结构：

```json
{
  "type": "message_type",
  "data": { }
}
```
**data** 为信息的附带信息，根据类型填写

**message_type** 为消息类型：有以下类型

---

## 客户端 → 服务器消息

### 1. 请求游戏状态 (RequestGameState)

**用途：** 获取当前游戏的全局状态

**消息格式：**
```json
{
  "type": "request_game_state",
  "data": {}
}
```

**服务器响应：** `GameStateResponse`为游戏状态

---

### 2. 请求玩家信息 (RequestPlayerInfo)

**用途：** 获取指定玩家的详细信息

**消息格式：**
```json
{
  "type": "request_player_info",
  "data": {
    "uuid": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

**字段说明：**
- `uuid`: 玩家的UUID（字符串格式）

**服务器响应：** `PlayerInfoResponse`为该玩家的信息

---

### 3. 发送投资行动 (SendInvestment)

**用途：** 在投资阶段执行各种投资行动

**消息格式：**
```json
{
  "type": "send_investment",
  "data": {
    "action": { }
  }
}
```

**action字段详解：**

#### 3.1 探索 (Explore)
```json
{
  "action": {
    "type": "explore",
    "data": {}
  }
}
```

#### 3.2 交换 (Exchange)
```json
{
  "action": {
    "type": "exchange",
    "data": {}
  }
}
```

#### 3.3 建造 (Build)
```json
{
  "action": {
    "type": "build",
    "data": {
      "building": "farm"
    }
  }
}
```

**建筑类型：**
- `"farm"` - 农场
- `"super_farm"` - 超级农场
- `"miner"` - 矿工
- `"super_miner"` - 超级矿工
- `"bank"` - 银行
- `"cannon"` - 大炮
- `"pickaxe"` - 镐
- `"lumber"` - 伐木场

#### 3.4 粉碎矿石 (CrushOre)
```json
{
  "action": {
    "type": "crush_ore",
    "data": {}
  }
}
```

#### 3.5 银行存钱 (StoreMoney)
```json
{
  "action": {
    "type": "store_money",
    "data": {
      "item": "gold",
      "count": 5
    }
  }
}
```

**资源类型：**
- `"gold"` - 金
- `"diamond"` - 钻石
- `"wood"` - 木材
- `"ore"` - 矿石
- `"food"` - 食物
- `"iron"` - 铁

#### 3.6 结束投资 (End)
```json
{
  "action": {
    "type": "end",
    "data": {}
  }
}
```

**服务器响应：** `InvestmentResult`

---

### 4. 发送投标 (SendBidding)

**用途：** 在投标阶段提交投标金额

**消息格式：**
```json
{
  "type": "send_bidding",
  "data": {
    "bidding": 3
  }
}
```

**字段说明：**
- `bidding`: 投标的行动点数量（必须 ≥ 1）

**服务器响应：** `BiddingResult`

---

### 5. 发送争夺行动 (SendContending)

**用途：** 在争夺阶段从市场拿取资源

**消息格式：**

#### 5.1 拿取资源 (Take)
```json
{
  "type": "send_contending",
  "data": {
    "action": {
      "type": "take",
      "data": {
        "index": 0,
        "item": "gold"
      }
    }
  }
}
```

**字段说明：**
- `index`: 市场中资源的索引位置（从0开始）
- `item`: 资源类型（必须与市场中该位置的资源匹配）

#### 5.2 结束争夺 (End)
```json
{
  "type": "send_contending",
  "data": {
    "action": {
      "type": "end",
      "data": {}
    }
  }
}
```

**服务器响应：** `ContendingResult`

---

## 服务器 → 客户端消息

### 1. 广播消息 (Broadcast)

服务器会向所有玩家广播以下消息：

#### 1.1 阶段变更 (PhaseChanged)
```json
{
  "type": "broadcast",
  "target": {
    "type": "phase_changed",
    "target": {
      "epoch": 1,
      "phase": 2
    }
  }
}
```

**字段说明：**
- `epoch`: 当前纪元（1-10）
- `phase`: 当前阶段（1-4）

#### 1.2 游戏开始 (GameStart)
```json
{
  "type": "broadcast",
  "target": {
    "type": "game_start",
    "target": {}
  }
}
```

#### 1.3 心跳 (HeartBeat)
```json
{
  "type": "broadcast",
  "target": {
    "type": "heart_beat",
    "target": {
      "state": {
        "players": ["uuid1", "uuid2", "uuid3", "uuid4"],
        "market": ["gold", "wood", "iron", ...],
        "epoch": 1,
        "phase": 1,
        "values": {
          "gold": 6,
          "diamond": 8,
          "wood": 2,
          "ore": 3,
          "food": 1,
          "iron": 2
        },
        "started": true
      },
      "interval": 10
    }
  }
}
```

**字段说明：**
- `state`: 游戏状态对象
- `interval`: 心跳间隔（秒）

#### 1.4 市场为空 (MarketEmpty)
```json
{
  "type": "broadcast",
  "target": {
    "type": "market_empty",
    "target": {}
  }
}
```

#### 1.5 游戏结束 (GameOver)
```json
{
  "type": "broadcast",
  "target": {
    "type": "game_over",
    "target": {
      "player_total_value": {
        "uuid1": 150,
        "uuid2": 120,
        "uuid3": 180,
        "uuid4": 95
      }
    }
  }
}
```

**字段说明：**
- `player_total_value`: 每个玩家的最终得分

#### 1.6 投标排序 (BiddingSorted)
```json
{
  "type": "broadcast",
  "target": {
    "type": "bidding_sorted",
    "target": {
      "order": ["uuid3", "uuid1", "uuid2", "uuid4"]
    }
  }
}
```

**字段说明：**
- `order`: 按投标金额排序的玩家UUID列表（从高到低）

#### 1.7 其他玩家投标 (OthersBidding)
```json
{
  "type": "broadcast",
  "target": {
    "type": "others_bidding",
    "target": {
      "uuid": "uuid1",
      "bidding": 5
    }
  }
}
```

**字段说明：**
- `uuid`: 投标玩家的UUID
- `bidding`: 投标金额

#### 1.8 其他玩家争夺 (OthersContending)
```json
{
  "type": "broadcast",
  "target": {
    "type": "others_contending",
    "target": {
      "action": {
        "type": "take",
        "data": {
          "index": 2,
          "item": "gold"
        }
      }
    }
  }
}
```

**字段说明：**
- `action`: 其他玩家执行的争夺行动（可以是 `take` 或 `end`）
  - 如果是 `take`：包含 `index`（市场索引）和 `item`（资源类型）
  - 如果是 `end`：表示该玩家结束争夺

#### 1.9 资源价值变化 (ValueChanged)
```json
{
  "type": "broadcast",
  "target": {
    "type": "value_changed",
    "target": {
      "item": "gold",
      "now": 7
    }
  }
}
```

**字段说明：**
- `item`: 资源类型
- `now`: 新的价值

#### 1.10 事件选择 (EventChosen)
```json
{
  "type": "broadcast",
  "target": {
    "type": "event_chosen",
    "target": {
      "event": "pirate_attack"
    }
  }
}
```

**事件类型：**
- `"pirate_attack"` - 海盗袭击
- `"crop_bonus"` - 丰收
- `"ap_bonus"` - 行动点奖励
- `"famine"` - 饥荒

---

### 2. 需要数据 (DataRequired)

**用途：** 通知客户端当前阶段需要玩家输入

```json
{
  "type": "data_required",
  "target": {
    "epoch": 1,
    "phase": 1
  }
}
```

**阶段说明：**
- `phase: 1` - 投资阶段，需要发送 `SendInvestment`
- `phase: 2` - 投标阶段，需要发送 `SendBidding`
- `phase: 3` - 争夺阶段，需要发送 `SendContending`
- `phase: 4` - 特殊事件阶段（无需玩家输入）

---

### 3. 游戏状态响应 (GameStateResponse)

```json
{
  "type": "game_state_response",
  "target": {
    "state": {
      "players": ["uuid1", "uuid2", "uuid3", "uuid4"],
      "market": ["gold", "wood", "iron", "diamond", "food"],
      "epoch": 1,
      "phase": 1,
      "values": {
        "gold": 6,
        "diamond": 8,
        "wood": 2,
        "ore": 3,
        "food": 1,
        "iron": 2
      },
      "started": true
    }
  }
}
```

**字段说明：**
- `players`: 所有玩家的UUID列表
- `market`: 市场中的资源列表（按索引顺序）
- `epoch`: 当前纪元
- `phase`: 当前阶段
- `values`: 各资源的当前价值
- `started`: 游戏是否已开始

---

### 4. 玩家信息响应 (PlayerInfoResponse)

```json
{
  "type": "player_info_response",
  "target": {
    "uuid": "uuid1",
    "player": {
      "name": "Alice",
      "action_points": 5,
      "resources": {
        "gold": 3,
        "diamond": 1,
        "wood": 5,
        "ore": 2,
        "food": 8,
        "iron": 4
      },
      "buildings": ["farm", "miner", "bank"],
      "bank_money": 50
    }
  }
}
```

**字段说明：**
- `uuid`: 玩家UUID
- `name`: 玩家名称
- `action_points`: 当前行动点
- `resources`: 各资源的数量
- `buildings`: 拥有的建筑列表
- `bank_money`: 银行存款

---

### 5. UUID通知 (UuidNotice)

**用途：** 连接成功后，服务器分配给客户端的UUID

```json
{
  "type": "uuid_notice",
  "target": {
    "uuid": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

**重要：** 客户端应保存此UUID，用于后续的 `RequestPlayerInfo` 请求。

---

### 6. 投资结果 (InvestmentResult)

```json
{
  "type": "investment_result",
  "target": {
    "action": {
      "type": "explore",
      "data": {}
    },
    "error": false,
    "reason": null
  }
}
```

**成功示例：**
```json
{
  "type": "investment_result",
  "target": {
    "action": {
      "type": "build",
      "data": {
        "building": "farm"
      }
    },
    "error": false
  }
}
```

**失败示例：**
```json
{
  "type": "investment_result",
  "target": {
    "action": {
      "type": "build",
      "data": {
        "building": "farm"
      }
    },
    "error": true,
    "reason": {
      "type": "no_enough_materials",
      "target": {
        "need_items": {
          "iron": 3,
          "wood": 2
        },
        "need_buildings": {}
      }
    }
  }
}
```

**错误类型：**

#### 6.1 行动点不足 (NoEnoughActionPoints)
```json
{
  "type": "no_enough_action_points",
  "target": {
    "need": 3
  }
}
```

#### 6.2 食物不足 (NoEnoughFood)
```json
{
  "type": "no_enough_food",
  "target": {
    "need": 1
  }
}
```

#### 6.3 没有矿工或超级矿工 (DontHaveMinerOrSuperMiner)
```json
{
  "type": "dont_have_miner_or_super_miner",
  "target": {}
}
```

#### 6.4 超过次数限制 (LimitsExceeded)
```json
{
  "type": "limits_exceeded",
  "target": {
    "limit": 5
  }
}
```

#### 6.5 行动未启用 (ActionIsNotEnabled)
```json
{
  "type": "action_is_not_enabled",
  "target": {}
}
```

#### 6.6 建筑未启用 (BuildingIsNotEnabled)
```json
{
  "type": "building_is_not_enabled",
  "target": {}
}
```

#### 6.7 材料不足 (NoEnoughMaterials)
```json
{
  "type": "no_enough_materials",
  "target": {
    "need_items": {
      "iron": 3,
      "wood": 2
    },
    "need_buildings": {
      "farm": 1
    }
  }
}
```

#### 6.8 矿石不足 (NoEnoughOre)
```json
{
  "type": "no_enough_ore",
  "target": {
    "need": 1
  }
}
```

#### 6.9 物品不足 (NoEnoughItem)
```json
{
  "type": "no_enough_item",
  "target": {
    "need": {
      "gold": 5
    }
  }
}
```

---

### 7. 建筑工作通知 (BuildingWorked)

**用途：** 通知玩家某个建筑已激活并产生资源

```json
{
  "type": "building_worked",
  "target": {
    "building": "farm"
  }
}
```

---

### 8. 投标结果 (BiddingResult)

**成功示例：**
```json
{
  "type": "bidding_result",
  "target": {
    "bidding": 5,
    "error": false
  }
}
```

**失败示例：**
```json
{
  "type": "bidding_result",
  "target": {
    "bidding": 10,
    "error": true,
    "reason": {
      "type": "no_enough_action_points",
      "target": {
        "need": 10
      }
    }
  }
}
```

**错误类型：**

#### 8.1 行动点不足 (NoEnoughActionPoints)
```json
{
  "type": "no_enough_action_points",
  "target": {
    "need": 5
  }
}
```

#### 8.2 投标无效 (BiddingNotValid)
```json
{
  "type": "bidding_not_valid",
  "target": {
    "max": 0,
    "min": 1
  }
}
```

---

### 9. 争夺结果 (ContendingResult)

**成功示例（拿取资源）：**
```json
{
  "type": "contending_result",
  "target": {
    "action": {
      "type": "take",
      "data": {
        "index": 0,
        "item": "gold"
      }
    },
    "error": false
  }
}
```

**成功示例（结束争夺）：**
```json
{
  "type": "contending_result",
  "target": {
    "action": {
      "type": "end",
      "data": {}
    },
    "error": false
  }
}
```

**失败示例：**
```json
{
  "type": "contending_result",
  "target": {
    "action": {
      "type": "take",
      "data": {
        "index": 0,
        "item": "gold"
      }
    },
    "error": true,
    "reason": {
      "type": "item_not_found",
      "target": {}
    }
  }
}
```

**错误类型：**

#### 9.1 行动点不足 (NoEnoughActionPoints)
```json
{
  "type": "no_enough_action_points",
  "target": {
    "need": 5
  }
}
```

#### 9.2 物品未找到 (ItemNotFound)
```json
{
  "type": "item_not_found",
  "target": {}
}
```

---

## 客户端开发流程

### 1. 连接阶段

```
1. 建立WebSocket连接到 ws://{host}:{port}/ws/{player_name}
2. 接收 UuidNotice 消息，保存UUID
3. 等待 GameStart 广播
```

### 2. 游戏循环

每个纪元包含4个阶段，客户端需要根据 `DataRequired` 消息响应：

#### 阶段1：投资阶段
```
1. 接收 DataRequired (phase: 1)
2. 接收 BuildingWorked 消息（如果有建筑）
3. 循环发送 SendInvestment 消息执行投资行动
4. 发送 End 行动结束投资阶段
5. 接收 InvestmentResult 确认每个行动
```

#### 阶段2：投标阶段
```
1. 接收 DataRequired (phase: 2)
2. 发送 SendBidding 消息提交投标
3. 接收 BiddingResult 确认投标
4. 接收 OthersBidding 广播（其他玩家的投标）
5. 接收 BiddingSorted 广播（投标排序）
```

#### 阶段3：争夺阶段
```
1. 接收 DataRequired (phase: 3)
2. 根据投标顺序轮流行动
3. 发送 SendContending 消息拿取资源或结束
4. 接收 ContendingResult 确认行动
5. 接收 OthersContending 广播（其他玩家的行动）
6. 接收 ValueChanged 广播（资源价值变化）
```

#### 阶段4：特殊事件阶段
```
1. 接收 EventChosen 广播（随机事件）
2. 无需玩家输入，自动处理
```

### 3. 游戏结束

```
1. 接收 GameOver 广播
2. 显示最终得分
3. 断开连接或等待新游戏
```

---

## 心跳机制

服务器每10秒发送一次心跳消息，包含完整的游戏状态。客户端可以：
- 使用心跳更新UI
- 检测连接状态
- 同步游戏状态

---

## 状态查询

客户端可以随时发送以下请求：

### 查询游戏状态
```json
{
  "type": "request_game_state",
  "data": {}
}
```

### 查询玩家信息
```json
{
  "type": "request_player_info",
  "data": {
    "uuid": "your_uuid_here"
  }
}
```

---

## 错误处理建议

1. **连接失败**：检查服务器地址、端口、token
2. **消息发送失败**：检查JSON格式、字段类型
3. **行动被拒绝**：根据 `reason` 字段提示用户
4. **超时**：重新发送请求或重连
5. **心跳丢失**：检测连接状态，必要时重连

---

## 完整示例：投资阶段流程

### 客户端发送
```json
// 1. 探索
{
  "type": "send_investment",
  "data": {
    "action": {
      "type": "explore",
      "data": {}
    }
  }
}

// 2. 建造农场
{
  "type": "send_investment",
  "data": {
    "action": {
      "type": "build",
      "data": {
        "building": "farm"
      }
    }
  }
}

// 3. 结束投资
{
  "type": "send_investment",
  "data": {
    "action": {
      "type": "end",
      "data": {}
    }
  }
}
```

### 服务器响应
```json
// 1. 探索成功
{
  "type": "investment_result",
  "target": {
    "action": {
      "type": "explore",
      "data": {}
    },
    "error": false
  }
}

// 2. 建造失败（材料不足）
{
  "type": "investment_result",
  "target": {
    "action": {
      "type": "build",
      "data": {
        "building": "farm"
      }
    },
    "error": true,
    "reason": {
      "type": "no_enough_materials",
      "target": {
        "need_items": {
          "iron": 3,
          "wood": 2
        },
        "need_buildings": {}
      }
    }
  }
}

// 3. 结束成功
{
  "type": "investment_result",
  "target": {
    "action": {
      "type": "end",
      "data": {}
    },
    "error": false
  }
}
```

---

## 技术细节

### JSON序列化规范
- 所有枚举使用 `snake_case` 命名
- 使用 `tagged union` 结构（`type` + `data`/`target`）
- UUID使用标准字符串格式

### WebSocket子协议
- 使用标准WebSocket协议
- 消息类型：Text（JSON字符串）
- 编码：UTF-8

### 连接生命周期
1. 连接建立
2. 接收UUID通知
3. 等待游戏开始
4. 游戏循环（10个纪元）
5. 游戏结束
6. 连接关闭

---

## 附录：资源类型映射表

| 英文名称 | 中文名称 | JSON值 |
|---------|---------|--------|
| Gold | 金 | `"gold"` |
| Diamond | 钻石 | `"diamond"` |
| Wood | 木材 | `"wood"` |
| Ore | 矿石 | `"ore"` |
| Food | 食物 | `"food"` |
| Iron | 铁 | `"iron"` |

## 附录：建筑类型映射表

| 英文名称 | 中文名称 | JSON值 |
|---------|---------|--------|
| Farm | 农场 | `"farm"` |
| Super Farm | 超级农场 | `"super_farm"` |
| Miner | 矿工 | `"miner"` |
| Super Miner | 超级矿工 | `"super_miner"` |
| Bank | 银行 | `"bank"` |
| Cannon | 大炮 | `"cannon"` |
| Pickaxe | 镐 | `"pickaxe"` |
| Lumber | 伐木场 | `"lumber"` |

## 附录：事件类型映射表

| 英文名称 | 中文名称 | JSON值 |
|---------|---------|--------|
| Pirate Attack | 海盗袭击 | `"pirate_attack"` |
| Crop Bonus | 丰收 | `"crop_bonus"` |
| AP Bonus | 行动点奖励 | `"ap_bonus"` |
| Famine | 饥荒 | `"famine"` |

---

*本协议文档基于代码版本：commit 6bf6676*
*协议定义位置：src/lib.rs:2456-2704*