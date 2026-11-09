# Edge 语音提示方案（Pi 事件对齐）

## 用法

壳配置 → 通知 Notify → 打开「启用全事件语音」。

会自动：

1. 安装 `~/.pi/agent/extensions/armor-notify.js`  
2. 同步语音包到 `~/.pi/agent/armor-sounds/<packId>/`  
3. 写入 `~/.pi/agent/armor-notify.json`  
4. 关掉旧的 `done-sound.json`，避免和 `pi-notify-sound` 叠声  

之后 **新的 Pi 会话** 会按事件播语音。

## 事件与文案

| 事件 / 文件 | 文案 |
| --- | --- |
| session_start | 会话已开始 |
| session_shutdown | 会话已结束 |
| agent_start | 开始处理 |
| agent_settled | 任务已完成 |
| agent_aborted | 已中止 |
| turn_start | 收到新消息 |
| turn_end | 本轮回复完成 |
| tool_call | 正在调用工具 |
| tool_result | 工具已返回 |
| tool_error | 工具执行失败 |
| ui_prompt_start | 需要您确认 |
| session_before_compact | 正在压缩上下文 |

音色：`zh-CN-XiaoxiaoNeural`。

## 重新生成

```bat
C:\dev\python312\python.exe E:\workspace\pi-armor\ops\gen-edge-voice-pack.py
```

## 说明

- 网页 `browserNotifications` 仍是系统通知音，与本方案分开。  
- 工具相关提示较密，可先在配置页试听再决定是否长期开启。  
