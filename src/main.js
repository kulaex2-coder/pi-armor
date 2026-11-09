const { invoke } = window.__TAURI__.core;

const frame = document.querySelector("#ui");
const boot = document.querySelector("#boot");
const bootMessage = document.querySelector("#boot-message");
const powerBtn = document.querySelector("#power");
const settingsBtn = document.querySelector("#settings");
const panelSettings = document.querySelector("#panel-settings");
const settingsSide = document.querySelector("#settings-side");
const settingsMain = document.querySelector("#settings-main");
const settingsHint = document.querySelector("#settings-hint");
const settingsBack = document.querySelector("#settings-back");
const settingsRefresh = document.querySelector("#settings-refresh");
const maxBtn = document.querySelector(".topbar__maximize");
const drag = document.querySelector("#drag");
const AUTO_KEY = "pi-armor.autoStart";

const iconPlay =
  '<svg viewBox="0 0 24 24" aria-hidden="true"><polygon points="8 5 19 12 8 19"></polygon></svg>';
const iconPause =
  '<svg viewBox="0 0 24 24" aria-hidden="true"><line x1="9" y1="6" x2="9" y2="18"></line><line x1="15" y1="6" x2="15" y2="18"></line></svg>';
const iconMax =
  '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="6" y="6" width="12" height="12" rx="2"></rect></svg>';
const iconRestore =
  '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="4" y="8" width="12" height="12" rx="2"></rect><path d="M8 8V6a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-2"></path></svg>';

let running = false;
let appearancePref = "system";
let settingsOpen = false;
let settingsTab = "notify";
let snapshot = {
  info: null,
  web: null,
  sound: null,
  webSettings: null,
  extensions: [],
  appearance: "system",
  theme: null,
  content: null,
};

const NAV = [
  { id: "launch", group: "常用", label: "启动", blurb: "打开窗口时的启动行为" },
  { id: "theme", group: "常用", label: "主题", blurb: "中间网页的配色" },
  { id: "notify", group: "常用", label: "通知 Notify", blurb: "armor-notify 全事件语音 + 网页系统通知" },
  { id: "web", group: "运行时", label: "Web UI", blurb: "内嵌网页与启动参数" },
  { id: "extensions", group: "运行时", label: "扩展板块", blurb: "壳里登记的外挂板块" },
  { id: "paths", group: "数据", label: "路径与配置", blurb: "样式、音色等内容的保存位置" },
  { id: "about", group: "数据", label: "关于", blurb: "壳版本与当前外观" },
];

function errText(err) {
  if (typeof err === "string") return err;
  if (err && typeof err.message === "string") return err.message;
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}

function windowLabel() {
  const meta = window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.metadata;
  return (meta && meta.currentWindow && meta.currentWindow.label) || "main";
}

function winInvoke(command) {
  return invoke("plugin:window|" + command, { label: windowLabel() });
}

function setBootMessage(text) {
  bootMessage.textContent = text;
}

function setPower(mode, title) {
  powerBtn.innerHTML = mode === "on" ? iconPause : iconPlay;
  powerBtn.title = title;
  powerBtn.setAttribute("aria-label", title);
  powerBtn.disabled = mode === "busy";
}

function applyAppearance(pref) {
  appearancePref = pref || "system";
  const dark =
    appearancePref === "dark" ||
    (appearancePref !== "light" && matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.dataset.appearance = dark ? "dark" : "light";
}

async function syncAppearance() {
  try {
    applyAppearance(await invoke("web_appearance"));
  } catch {
    applyAppearance("system");
  }
}

function showBoot() {
  boot.hidden = false;
}

function showWeb(state) {
  running = !!state.running;
  if (running && state.url) {
    if (frame.getAttribute("src") !== state.url) frame.src = state.url;
    boot.hidden = true;
    setPower("on", "暂停");
    syncAppearance();
    return;
  }
  running = false;
  frame.removeAttribute("src");
  showBoot();
  setPower("off", "启动");
  setBootMessage(state.message || "Web UI 还没启动。点右上角图标开始。");
  applyAppearance("system");
}

async function loadInfo() {
  const info = await invoke("launch_info");
  document.querySelector("#boot-name").textContent = info.name;
  document.querySelector("#boot-version").textContent = "版本 " + info.version;
  const list = document.querySelector("#boot-params");
  const rows = [
    ["端口", String(info.port)],
    ["浏览器", info.openBrowser ? "自动打开" : "不自动打开"],
    ["命令", info.command],
  ];
  list.replaceChildren();
  for (const [name, value] of rows) {
    const dt = document.createElement("dt");
    dt.textContent = name;
    const dd = document.createElement("dd");
    dd.textContent = value;
    list.append(dt, dd);
  }
  setBootMessage(info.commandOk ? "Web UI 还没启动。点右上角图标开始。" : info.command);
  return info;
}

async function start() {
  setPower("busy", "启动中");
  setBootMessage("正在启动…");
  showBoot();
  try {
    showWeb(await invoke("start_web_ui"));
  } catch (err) {
    running = false;
    showBoot();
    setPower("off", "启动失败");
    setBootMessage(errText(err));
  }
}

async function stop() {
  powerBtn.disabled = true;
  try {
    showWeb(await invoke("stop_web_ui"));
  } catch (err) {
    setPower("on", errText(err));
  }
}

async function syncMaximize() {
  try {
    const maximized = await winInvoke("is_maximized");
    maxBtn.innerHTML = maximized ? iconRestore : iconMax;
    const label = maximized ? "向下还原" : "最大化";
    maxBtn.title = label;
    maxBtn.setAttribute("aria-label", label);
  } catch (err) {
    maxBtn.title = errText(err);
  }
}

function bindWindowControls() {
  document.querySelector(".topbar__minimize").addEventListener("click", () => {
    winInvoke("minimize").catch((err) => {
      document.querySelector(".topbar__minimize").title = errText(err);
    });
  });
  maxBtn.addEventListener("click", () => {
    winInvoke("toggle_maximize")
      .then(() => syncMaximize())
      .catch((err) => {
        maxBtn.title = errText(err);
      });
  });
  document.querySelector(".topbar__close").addEventListener("click", () => {
    winInvoke("close").catch((err) => {
      document.querySelector(".topbar__close").title = errText(err);
    });
  });
  drag.addEventListener("mousedown", (event) => {
    if (event.button !== 0 || event.detail > 1) return;
    winInvoke("start_dragging").catch((err) => {
      drag.title = errText(err);
    });
  });
  drag.addEventListener("dblclick", () => {
    maxBtn.click();
  });
  const events = window.__TAURI__ && window.__TAURI__.event;
  if (events && typeof events.listen === "function") {
    events.listen("tauri://resize", () => {
      syncMaximize();
    });
  }
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    if (appearancePref === "system") applyAppearance("system");
  });
  setInterval(() => {
    if (running) syncAppearance();
  }, 2000);
  syncMaximize();
}

function row(title, detail, control) {
  const el = document.createElement("div");
  el.className = "settings-row";
  const lab = document.createElement("div");
  lab.className = "settings-lab";
  const strong = document.createElement("strong");
  strong.textContent = title;
  lab.append(strong);
  if (detail) {
    const span = document.createElement("span");
    span.textContent = detail;
    lab.append(span);
  }
  el.append(lab, control);
  return el;
}

function textValue(text) {
  const span = document.createElement("span");
  span.className = "settings-value";
  span.textContent = text == null || text === "" ? "—" : String(text);
  return span;
}

function switchControl(on, onChange) {
  const btn = document.createElement("button");
  btn.type = "button";
  btn.className = "settings-sw" + (on ? " on" : "");
  btn.setAttribute("aria-pressed", String(!!on));
  btn.addEventListener("click", () => onChange(!btn.classList.contains("on")));
  return btn;
}

function section(title) {
  const el = document.createElement("div");
  el.className = "settings-sec";
  el.textContent = title;
  return el;
}

function setHint(text) {
  settingsHint.textContent = text;
}

async function loadSnapshot() {
  const [info, web, sound, webSettings, extensions, appearance, theme, content] = await Promise.all([
    invoke("launch_info").catch((err) => ({ error: errText(err) })),
    invoke("web_status").catch((err) => ({ error: errText(err) })),
    invoke("sound_status").catch((err) => ({ error: errText(err) })),
    invoke("web_settings").catch((err) => ({ available: false, message: errText(err) })),
    invoke("list_extensions").catch(() => []),
    invoke("web_appearance").catch(() => "system"),
    invoke("theme_status").catch((err) => ({ error: errText(err), themes: [] })),
    invoke("content_status").catch((err) => ({ error: errText(err) })),
  ]);
  snapshot = { info, web, sound, webSettings, extensions, appearance, theme, content };
  return snapshot;
}

function renderLaunch() {
  const auto = localStorage.getItem(AUTO_KEY) !== "0";
  const web = snapshot.web || {};
  settingsMain.append(
    section("行为"),
    row(
      "打开时自动启动",
      "勾选后打开壳会自动拉起 Web UI",
      switchControl(auto, (next) => {
        localStorage.setItem(AUTO_KEY, next ? "1" : "0");
        renderSettingsTab("launch");
        setHint(next ? "已开启自动启动" : "已关闭自动启动");
      }),
    ),
    section("状态"),
    row("Web UI 运行中", "", textValue(web.running ? "是" : "否")),
    row("是否本窗口启动", "", textValue(web.owned ? "是" : "否")),
    row("状态说明", "", textValue(web.message || web.error || "—")),
  );
}

function renderNotify() {
  const sound = snapshot.sound || {};
  const webSettings = snapshot.webSettings || {};

  settingsMain.append(section("扩展 pi-notify-sound"));
  if (sound.error) {
    settingsMain.append(row("读取失败", "", textValue(sound.error)));
  } else {
    const installBtn = document.createElement("button");
    installBtn.type = "button";
    installBtn.className = "settings-action";
    installBtn.textContent = sound.pluginInstalled ? "重新检查安装" : "安装扩展";
    installBtn.addEventListener("click", () => {
      setHint("正在安装 / 检查…");
      invoke("ensure_sound_plugin")
        .then((msg) =>
          invoke("sound_status").then((status) => {
            snapshot.sound = status;
            renderSettingsTab("notify");
            setHint(msg);
          }),
        )
        .catch((err) => setHint(errText(err)));
    });

    const select = document.createElement("select");
    select.className = "settings-select";
    for (const pack of sound.packs || []) {
      const opt = document.createElement("option");
      opt.value = pack.id;
      opt.textContent = pack.title;
      if (pack.id === sound.packId) opt.selected = true;
      select.append(opt);
    }
    select.disabled = !sound.enabled;
    select.addEventListener("change", () => {
      invoke("sound_set_pack", { packId: select.value })
        .then((status) => {
          snapshot.sound = status;
          renderSettingsTab("notify");
          setHint("已切换音色");
        })
        .catch((err) => setHint(errText(err)));
    });

    const preview = document.createElement("button");
    preview.type = "button";
    preview.className = "settings-action";
    preview.textContent = "试听完成音";
    preview.disabled = !sound.enabled;
    preview.addEventListener("click", () => {
      invoke("sound_preview", { packId: select.value || null, clipId: "agent_settled" })
        .then(() => setHint("已试听：任务已完成"))
        .catch((err) => setHint(errText(err)));
    });

    settingsMain.append(
      row("扩展", "写入 ~/.pi/agent/extensions/armor-notify.js", textValue("armor-notify")),
      row("扩展已安装", sound.pluginMessage || "", textValue(sound.pluginInstalled ? "是" : "否")),
      row("安装 / 检查", "复制扩展到 pi agent 目录", installBtn),
      row(
        "启用全事件语音",
        "开启后同步语音包并挂到 Pi 生命周期事件",
        switchControl(!!sound.enabled, (next) => {
          invoke("sound_set_enabled", { enabled: next })
            .then((status) => {
              snapshot.sound = status;
              renderSettingsTab("notify");
              setHint(next ? "语音通知已开启，新会话起生效" : "语音通知已关闭");
            })
            .catch((err) => setHint(errText(err)));
        }),
      ),
      row("音色 / 语音包", "推荐 Edge 语音 · 晓晓（事件对齐）", select),
      row("试听完成音", "agent_settled · 任务已完成", preview),
      row("配置文件", "", textValue(sound.configPath || "—")),
      row("当前包 ID", "", textValue(sound.packId || "—")),
    );

    const clips = sound.clips || [];
    if (clips.length) {
      settingsMain.append(section("Edge / 包内提示词"));
      for (const clip of clips) {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = "settings-action";
        btn.textContent = "试听";
        btn.addEventListener("click", () => {
          invoke("sound_preview", { packId: sound.packId || select.value, clipId: clip.id })
            .then(() => setHint("已试听：" + clip.text))
            .catch((err) => setHint(errText(err)));
        });
        settingsMain.append(row(clip.text, clip.id + " · " + (clip.path || ""), btn));
      }
    }
  }

  settingsMain.append(section("网页系统通知"));
  if (!webSettings.available) {
    settingsMain.append(
      row("browserNotifications", webSettings.message || "Web UI 未启动", textValue("不可用")),
    );
  } else {
    settingsMain.append(
      row(
        "浏览器通知",
        "Web UI 的 browserNotifications；响的是系统通知音，不是扩展音色",
        switchControl(!!webSettings.browserNotifications, (next) => {
          invoke("set_browser_notifications", { enabled: next })
            .then((status) => {
              snapshot.webSettings = status;
              renderSettingsTab("notify");
              setHint(next ? "已打开网页通知" : "已关闭网页通知");
            })
            .catch((err) => setHint(errText(err)));
        }),
      ),
      row("网页外观", "", textValue(webSettings.appearance || "—")),
      row("网页语言", "", textValue(webSettings.language || "—")),
    );
  }

  const note = document.createElement("p");
  note.className = "settings-note";
  note.textContent =
    "壳内打开「启用全事件语音」后，Pi 新会话会按事件播 Edge 语音。网页系统通知是另一条线，可能叠声。";
  settingsMain.append(note);
}

function reloadWeb() {
  const src = frame.getAttribute("src");
  if (!src) return;
  const url = new URL(src);
  url.searchParams.set("armor", String(Date.now()));
  frame.src = url.toString();
}

function renderTheme() {
  const theme = snapshot.theme || { themes: [] };
  if (theme.error) {
    settingsMain.append(row("读取失败", "", textValue(theme.error)));
    return;
  }
  const select = document.createElement("select");
  select.className = "settings-select";
  for (const item of theme.themes || []) {
    const opt = document.createElement("option");
    opt.value = item.id;
    opt.textContent = item.title;
    if (item.id === theme.id) opt.selected = true;
    select.append(opt);
  }
  select.addEventListener("change", () => {
    setHint("正在切换主题…");
    invoke("theme_set", { themeId: select.value })
      .then((status) => {
        snapshot.theme = status;
        reloadWeb();
        renderSettingsTab("theme");
        setHint("已换成" + status.title + "。中间页已刷新。");
      })
      .catch((err) => setHint(errText(err)));
  });
  settingsMain.append(
    section("中间网页"),
    row("主题", "小红书近期热门风格，选完立即换中间页", select),
    row("当前", "", textValue(theme.title)),
    row("样式文件", "在保存位置的 themes 里，改完刷新即可", textValue(theme.path)),
  );
}

function renderWeb() {
  const info = snapshot.info || {};
  const web = snapshot.web || {};
  settingsMain.append(
    section("连接"),
    row("地址", "", textValue(web.url || "—")),
    row("端口", "", textValue(info.port)),
    row("自动打开浏览器", "", textValue(info.openBrowser ? "是" : "否")),
    section("命令"),
    row("可执行文件", info.commandOk ? "已找到" : "未找到", textValue(info.command || "—")),
    section("外观跟随"),
    row("网页外观偏好", "从 /api/settings 读取", textValue(snapshot.appearance)),
    row("运行中", "", textValue(web.running ? "是" : "否")),
    row("消息", "", textValue(web.message || web.error || "—")),
  );
}

function renderExtensions() {
  const list = Array.isArray(snapshot.extensions) ? snapshot.extensions : [];
  settingsMain.append(section("已登记板块"));
  if (!list.length) {
    settingsMain.append(row("暂无板块", "后期有第二个板块时再做侧栏", textValue("—")));
    return;
  }
  for (const item of list) {
    settingsMain.append(row(item.title || item.id, item.id, textValue("已登记")));
  }
}

function refreshSavedContent(hint) {
  return Promise.all([
    invoke("content_status"),
    invoke("theme_status"),
    invoke("sound_status"),
  ]).then(([content, theme, sound]) => {
    snapshot.content = content;
    snapshot.theme = theme;
    snapshot.sound = sound;
    reloadWeb();
    renderSettingsTab("paths");
    setHint(hint);
  });
}

function renderPaths() {
  const content = snapshot.content || {};
  const sound = snapshot.sound || {};
  const info = snapshot.info || {};
  const actions = document.createElement("div");
  actions.style.display = "flex";
  actions.style.gap = "8px";
  actions.style.flexWrap = "wrap";
  actions.style.justifyContent = "flex-end";

  const openBtn = document.createElement("button");
  openBtn.type = "button";
  openBtn.className = "settings-action";
  openBtn.textContent = "打开";
  openBtn.addEventListener("click", () => {
    invoke("content_open_dir")
      .then((path) => setHint("已打开 " + path))
      .catch((err) => setHint(errText(err)));
  });

  const pickBtn = document.createElement("button");
  pickBtn.type = "button";
  pickBtn.className = "settings-action";
  pickBtn.textContent = "选择文件夹";
  pickBtn.addEventListener("click", () => {
    invoke("content_pick_folder")
      .then((picked) => {
        if (!picked) return;
        if (picked === content.dir) {
          setHint("这就是当前保存位置");
          return;
        }
        if (!window.confirm("样式和音色会复制到新文件夹，原来的留着不删。\n\n" + picked)) return;
        setHint("正在改保存位置…");
        return invoke("content_set_dir", { path: picked }).then(() =>
          refreshSavedContent("保存位置已改到新文件夹"),
        );
      })
      .catch((err) => setHint(errText(err)));
  });

  const resetBtn = document.createElement("button");
  resetBtn.type = "button";
  resetBtn.className = "settings-action";
  resetBtn.textContent = "恢复默认";
  resetBtn.addEventListener("click", () => {
    if (!window.confirm("恢复到文档里的 Pi Armor？当前文件夹里的样式和音色会复制回去。")) return;
    setHint("正在恢复默认位置…");
    invoke("content_reset_dir")
      .then(() => refreshSavedContent("已恢复到文档里的 Pi Armor"))
      .catch((err) => setHint(errText(err)));
  });
  actions.append(openBtn, resetBtn, pickBtn);

  settingsMain.append(
    section("保存位置"),
    row("当前文件夹", "默认是文档里的 Pi Armor。整个文件夹拷到另一台电脑的文档目录就能用", textValue(content.dir || content.error || "—")),
    row("操作", "换位置后，正在用的音色会改指到新文件夹", actions),
    row("样式", "themes", textValue(content.themesDir || "—")),
    row("音色", "sounds", textValue(content.soundsDir || "—")),
    section("壳与命令"),
    row("Web UI 命令", "", textValue(info.command || "—")),
    section("提示音指向"),
    row("语音配置", "pi 仍从这里读该播哪一套", textValue(sound.configPath || "—")),
    row("当前音色文件", "", textValue((sound.packs || []).find((p) => p.id === sound.packId)?.path || "—")),
  );
}

function renderAbout() {
  const info = snapshot.info || {};
  settingsMain.append(
    section("应用"),
    row("名称", "", textValue(info.name || "Pi Armor")),
    row("版本", "", textValue(info.version || "—")),
    row("当前外观", "", textValue(document.documentElement.dataset.appearance || "—")),
    row("网页外观偏好", "", textValue(snapshot.appearance || "—")),
  );
}

function renderSettingsTab(id) {
  settingsTab = id || settingsTab;
  const page = NAV.find((item) => item.id === settingsTab) || NAV[0];
  settingsTab = page.id;
  [...settingsSide.querySelectorAll("button")].forEach((btn) => {
    btn.classList.toggle("on", btn.dataset.id === settingsTab);
  });
  settingsMain.replaceChildren();
  const h2 = document.createElement("h2");
  h2.textContent = page.label;
  const sub = document.createElement("p");
  sub.className = "sub";
  sub.textContent = page.blurb;
  settingsMain.append(h2, sub);
  if (settingsTab === "launch") renderLaunch();
  else if (settingsTab === "theme") renderTheme();
  else if (settingsTab === "notify") renderNotify();
  else if (settingsTab === "web") renderWeb();
  else if (settingsTab === "extensions") renderExtensions();
  else if (settingsTab === "paths") renderPaths();
  else renderAbout();
}

function buildSettingsNav() {
  settingsSide.replaceChildren();
  let lastGroup = "";
  for (const page of NAV) {
    if (page.group !== lastGroup) {
      const cap = document.createElement("div");
      cap.className = "cap";
      cap.textContent = page.group;
      settingsSide.append(cap);
      lastGroup = page.group;
    }
    const btn = document.createElement("button");
    btn.type = "button";
    btn.dataset.id = page.id;
    btn.textContent = page.label;
    btn.addEventListener("click", () => renderSettingsTab(page.id));
    settingsSide.append(btn);
  }
}

async function openSettings() {
  settingsOpen = true;
  document.body.classList.add("settings-open");
  panelSettings.hidden = false;
  settingsBtn.setAttribute("aria-pressed", "true");
  drag.textContent = "Pi Armor · 配置";
  setHint("正在读取数据…");
  try {
    await loadSnapshot();
    renderSettingsTab(settingsTab);
    setHint("已载入当前数据");
  } catch (err) {
    setHint(errText(err));
  }
}

function closeSettings() {
  settingsOpen = false;
  document.body.classList.remove("settings-open");
  panelSettings.hidden = true;
  settingsBtn.setAttribute("aria-pressed", "false");
  drag.textContent = "Pi Armor";
}

function bindSettings() {
  buildSettingsNav();
  settingsBtn.addEventListener("click", () => {
    if (settingsOpen) closeSettings();
    else openSettings();
  });
  settingsBack.addEventListener("click", () => closeSettings());
  settingsRefresh.addEventListener("click", () => {
    setHint("正在刷新…");
    loadSnapshot()
      .then(() => {
        renderSettingsTab(settingsTab);
        setHint("已刷新");
      })
      .catch((err) => setHint(errText(err)));
  });
}

window.addEventListener("DOMContentLoaded", () => {
  setPower("off", "启动");
  powerBtn.addEventListener("click", () => {
    if (running) stop();
    else start();
  });
  bindSettings();
  bindWindowControls();
  invoke("sound_bootstrap").catch(() => {});
  const revealBoot = setTimeout(() => {
    if (!running && !settingsOpen) showBoot();
  }, 700);
  loadInfo()
    .catch((err) => setBootMessage(errText(err)))
    .then(() => invoke("web_status"))
    .then((state) => {
      clearTimeout(revealBoot);
      if (state.running) {
        showWeb(state);
        return;
      }
      showBoot();
      if (localStorage.getItem(AUTO_KEY) !== "0") start();
    })
    .catch((err) => {
      clearTimeout(revealBoot);
      showBoot();
      setBootMessage(errText(err));
    });
});
