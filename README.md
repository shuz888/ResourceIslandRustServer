# 资源岛争夺游戏服务端
### 游戏背景
你乘坐的船只在海上遇难，你和三位其他乘客侥幸存活并来到了这片岛屿，开始自己的生存之旅，你们发现岛上的资源是有限的，于是开始开采矿石，修建建筑来争夺这些物资并应对可能迎来的灾难。
### 游戏规则
详见[rule.md](https://github.com/shuz888/ResourceIslandRustServer/blob/main/.docs/rule.md)
### 关于服务端
#### TODO：
- [x] 基本服务端结构
- [x] 配置文件功能
- [x] 游戏主循环
- [ ] ~~tests~~
- [x] 编写游戏规则文档
- [x] 编写适配的客户端

服务器借助axum框架、serde库等实现服务、配置功能。

我建议你**不要更改服务器配置文件**，放在那里就好，否则无法适用rule.md的规则。

将很快发布第一个release。

#### 文件功能一览
##### src/
- main.rs：服务端初始化，加载配置，是入口文件。
- config.rs：负责定义配置结构体。
- enums.rs：存放枚举类型，包含服务器客户端互发消息的DTO。
- game.rs：游戏主循环。
- routes.rs：路由handler。
- structs.rs：存放结构体类型，定义游戏状态（State）结构体，玩家（Player）结构体等，包含DTO。
##### tests/
- test.rs：编写测试。
---
### 关于客户端的编写
服务端网络通信说明详见[protocol.md](https://github.com/shuz888/ResourceIslandRustServer/blob/main/.docs/protocol.md)

AI编写的客户端：目前没有发现什么问题，可以在[这里](https://github.com/shuz888/shuz888.github.io/blob/main/index.html)下载。

需要注意的是，要用请下载到本地，并且**不能直接点开**，要用类似`python -m http.server`这样的服务端挂一下，然后在浏览器访问服务端地址，否则无法连接（实测）。