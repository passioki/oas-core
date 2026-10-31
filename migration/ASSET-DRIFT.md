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
