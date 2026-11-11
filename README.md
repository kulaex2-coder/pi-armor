# Pi Armor

本机启动器。打开窗口即拉起官方 `pi-web-simple`，并在窗口里显示 http://127.0.0.1:5319 。

```
cd pi-armor
npm install
npm run tauri dev
```

需要本机已能执行 `pi-web-simple`，Node.js >= 22.19。启动页会检测 Node.js / Pi / Pi Web UI，并显示壳版本；缺项不会自动装，点「安装缺失依赖」并确认后才会安装。

已经在跑的 Web UI 只会接入，点「停止」不会关掉它。只有本窗口启动的进程，退出或点「停止」时才会结束。

窗口无系统边框。按住左侧标题拖动。右侧依次是设置、启动图标、最小化、最大化、关闭。播放图标是启动，暂停图标是已启动。没启动时中间显示项目名、版本和启动参数。顶栏颜色跟随 pi 的明亮或黑暗模式。

点齿轮进入壳专用配置页。通知页打开「启用全事件语音」后，会安装 `armor-notify`、同步 Edge 语音包，并在 Pi 新会话里按生命周期事件播报（开始/完成/工具/确认等）。说明见 `ops/edge-voice-pack.md`。网页系统通知是另一条线。

样式和音色放在文档目录的 `Pi Armor` 文件夹（`themes`、`sounds`）。在「路径与配置」里可以打开、另选文件夹或恢复默认。换电脑时把这个文件夹拷到新机器的文档目录即可。内嵌 Web UI 的显示/隐藏一律改主题 CSS，不要用脚本改中间页 DOM；见 `AGENTS.md`。

后期加功能：有第二个板块时再做侧栏。登记处在 `src-tauri/src/extensions.rs`。

## 许可

本项目采用 MIT 许可证，全文见 `LICENSE`。

`edge-zh-xiaoxiao` 的语音文件不纳入仓库。需要时按 `ops/edge-voice-pack.md` 在本机重新生成。
