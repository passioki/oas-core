# ASSET DRIFT — remote dev templates vs current CN client

> 证据时间：2026-11-01，emulator-5554，客户端 2.8.83.2384860（引擎 2.8.89）。
> 这是当前迁移 `🔄` 的最大系统性阻塞之一，不是单个任务逻辑问题。

## 观测

| asset | remote threshold | live best score | 现象 |
|---|---:|---:|---|
| `legacy_GameUi_I_CHECK_MAIN` | 0.95 | 0.745~0.832 | 主界面守卫时而 <0.95，已临时降到 0.75 |
| `legacy_GameUi_I_MAIN_GOTO_DAILY` | 0.7 | 0.558 | 花合战按钮不再匹配旧模板 |
| `legacy_GameUi_I_MAIN_GOTO_FRIENDS` | 0.7 | 0.448 | 好友入口不再匹配旧模板 |
| `庭院_探索灯笼`（core） | 0.88 | 0.640 | 主界面背景变化导致匹配漂移 |
| `legacy_DailyTrifles_O_LUCK_MSG` | 0.9 | OCR 行无“吉闻” | ROI/页面文本变化 |
| `legacy_TalismanPass_I_RED_POINT_LEVEL` | 0.7 | 误匹配成就红点 | 红点模板太通用，不能用于定位 |

## 已尝试

1. 自动 NCC 重采样主界面导航模板（旧模板在当前帧内搜索）→ 最高 0.556，失败；
2. 手工从当前主界面裁剪底部导航 → 模板匹配到错误位置（0.737，点到了玩家栏）；
3. 降低阈值 → 误匹配增加，不可接受。

## 额外发现：主界面 UI 可隐藏

2026-11-01 02:43 重新启动后，庭院主界面**没有底部导航栏**（只有右侧式神录/活动/召唤等），`I_MAIN_GOTO_DAILY/FRIENDS/MALL` 在这些截图上根本不可见；点击左下卷轴只切换了部分 UI。旧任务默认底部导航常显，当前客户端允许隐藏/切换 UI 状态。

因此实机入口失败有两类原因：

1. 模板漂移（图标重绘/位置变化）；
2. **主界面 UI 处于隐藏状态**，模板不存在。

迁移任务需要一个显式“恢复主界面 UI / 打开底部导航”的 bootstrap，或由用户在运行前保证 UI 显示；否则从庭院入口的任务会在第一步配置期失败。该 bootstrap 尚未实现，记入阻塞。

**可用的恢复动作（2026-11-01 实测）**：

- 点击主界面右下卷轴 `(1200,680)` 后，底部导航栏恢复（图鉴/珍旅居/纳物库/组队/阴阳寮/商店/花合战/好友/阴阳术/式神录）。
- 恢复 UI 后，`I_MAIN_GOTO_DAILY` 等旧模板重新可用（TalismanPass 实机入口 score 0.745 通过）。
- 已用恢复后的帧裁出 `ui_main_bar_guide` marker（当前底部“图鉴”图标），阈值 0.85；bootstrap `if not appear(ui_main_bar_guide) -> click_point(1200,680) -> wait` 实机 `_diag_ui` Done，marker score 1.000。
- TalismanPass / DailyTrifles 已加入该 bootstrap；TalismanPass 恢复导航后实机 Done。
- 注意：`ui_main_bar_guide` 是当前帧 UI marker，客户端/分辨率变化需重采；不要用旧 `I_MAIN_GOTO_DAILY` 判断 UI 可见（UI 可见时它也可能只有 0.579）。


## 手工裁剪的失败经验

在“底部导航已显示”的帧上按估算坐标裁剪 5 个导航图标，结果全部错位（裁到图鉴/角色/招财猫/探索等）。原因：底部图标间距、底栏实际 y 范围、动态角色遮挡都随界面状态变化，盲裁不可靠。

必须用编辑器的 ROI 工具，在**同一张目标帧**上逐图标画框并立即 `试识别`；不要用脚本按估算坐标批量裁。当前已有 `ui_main_bar_guide` 作为 UI-visible marker；其余导航模板仍需编辑器 ROI 重采。

## 测试方法论修正

2026-11-01 继续测试发现：很多“入口素材失败”是因为**测试从上一个任务残留的活动页/花札页起跑**，不是干净主界面。重启用 `force-stop + monkey launch` 回到主界面后再跑：

- DailyTrifles `friend_love` 实机 **Done**；
- DailyTrifles `store_sign` 实机 **Done**；
- 同一点 `(905,641)` 在干净主界面可用，残留在花札页时才会失败。

因此后续所有实机验证必须先 reset 到干净主界面（`am force-stop com.netease.onmyoji` + launch + 进入游戏），再计入证据；不能把残留页面失败误判为素材漂移。

## 结论

旧仓 `tasks/GameUi/page/*.png` 的导航模板不能靠阈值/自动重采样修复，必须在**当前客户端**用 ROI 工具重新截图绑定：

- 主界面：花合战、好友、商店、召唤、町中、活动；
- 页面守卫：主界面、花合战/每日、好友、商店、御灵、御魂；
- 动态文本：吉闻、门票、次数、时长。

## 推荐流程

1. 起当前客户端到目标页面；
2. 在编辑器/脚本里画模板 ROI（只框图标主体，避免背景/角色/呼吸动画）；
3. 用 `task lint` + `oasd run --dry-run` 验证 score；
4. 逐个任务替换 `legacy_GameUi_I_MAIN_GOTO_*` / 守卫；
5. 只有 score 稳定且实机 Done 才改 ✅。

## 影响任务（部分）

`TalismanPass`、`DailyTrifles`、`Hunt`、`AreaBoss`、`Exploration`、`GoryouRealm`、`Sougenbi`、`EvoZone`、`TrueOrochi`、`Orochi` 等所有从庭院入口的任务。

## 与行为等价的关系

逻辑迁移（meta/flow/asset id/守卫链）可以静态完成，但**页面识别不可用就无法实机行为等价**。在此表标定前，相关任务只能保持 🔄/⛔，不能标 ✅。
