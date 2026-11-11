# Pi Armor 项目规范

## Web UI 显示一律走主题 CSS

中间内嵌的 `pi-web-simple`，所有显示/隐藏/配色等视觉改动：

- **只**改主题 CSS（仓库 `themes/*.css`，或文档目录 `Pi Armor/themes`）
- 经代理注入的 `/__pi_armor/theme.css` 生效（`src-tauri/src/proxy.rs`）
- 选择器形态：`html [class*="_token_"]`

禁止用 iframe JS / DOM 补丁控制内嵌页 UI。壳自己的顶栏（icon、标题、窗口按钮）用 `src/`，与 Web UI 主题分开。

细则见 `.cursor/rules/web-ui-theme-css.mdc`。

## CSS 分区注释

`themes/*.css` 与 `src/styles.css` 按「令牌 / 控制元素」分区写注释，方便人手改色改形。主题文件顶部有目录；令牌块内再按中性色、背景、文字、品牌、描边、按钮等分子段；元素块用 `[侧栏]` `[新会话]` `[项目行]` `[输入区]` 标出对应 `class*="_token_"`。壳强制隐藏规则在 `proxy.rs` 的 `CHROME_CSS`，同样按元素分区注释。
