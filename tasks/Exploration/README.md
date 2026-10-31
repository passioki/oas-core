# Exploration（探索）

远程源：`runhey/OnmyojiAutoScript@dev` `tasks/Exploration`（commit `8b4dc66`）。

**状态：🔄** — 默认配置主链路（庭院→探索→第二十八章→打怪/BOSS/小纸人→返回）覆盖；高级选项与组队/滑屏尽头检测未迁移

## 行为（抽象）

1. 御魂切换（方式一/二）；加成勾选（金/经验 50/100）；`goto_page(page_mainline)`（探索大地图·主线 tab）
2. WORLD（`I_CHECK_EXPLORATION` 且无设置按钮）/ ENTRANCE（`I_E_EXPLORATION_CLICK`）/ MAIN（`I_E_SETTINGS_BUTTON` 或自动轮换图标）/ BATTLE_PREPARE / BATTLE_FIGHTING / TEAM / UNKNOWN
3. `run_solo()`：WORLD → 开右箭头→选章→进图；ENTRANCE → 点进入；MAIN → 自动轮换设置、捡宝箱、清剧情弹窗、领小纸人奖励、打 BOSS/UP 怪；无目标则 `_search_next_area()` 右滑找怪（滑到尽头判定 `is_swipe_unchanged`）
4. 组队：邀请好友（`invite_friend`：好友列表 OCR/分类 tab/选中/确认）+ 同 MAIN 循环 + 队友跑路检测（`I_TEAM_EMOJI`）
5. 在 WORLD 等邀请（`check_then_accept`）；接受后进战斗/领奖；超时 50s 退出
6. 点战斗图标→若仍在探索页说明没点到；`run_general_battle`→`minions_cnt += 1`
7. `minions_cnt >= minions_cnt`（默认 30）或超 `limit_time`（30min）；可选：突破票 >= 阈值时切 RealmRaid/MemoryScrolls（`scrolls_enable`）
8. 点 `I_BACK_YOLLOW`/`I_UI_BACK_BLUE`/`I_E_EXIT_CONFIRM` 回探索大世界；`post_process`：关加成、回庭院、`set_next_run`

## 目录 / 契约

```
tasks/Exploration/
├─ meta.json       # params/exits/capabilities/schedule_hint
├─ flow.json       # oas-flow DSL（仅既有 op）
├─ migration.json  # 源文件、差异、证据
└─ README.md
```

参数：`max_battles=30`（1~30）

## 运行 / 校验

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task Exploration   # => errors=0
.\target\release\oasd.exe run --task tasks\Exploration --param max_battles=30
```

## 已知限制

1. 仅第二十八章；其他章节滑屏选关未迁移
2. UP 定向找怪、自动候补、宝箱、剧情弹窗、加成、突破票联动未迁移
3. 滑屏尽头检测需帧差分原语，未迁移
4. 组队 LEADER/MEMBER + 好友邀请未迁移
5. 无本任务实机/回放证据（仅复用 tasks/探索 素材链）

详见 `migration/specs/Exploration.md`、`tasks/Exploration/migration.json`、`migration/status/Exploration.json`。

## 2026-11-01 静态修整（polish round）

- 修复：滑屏几何：上游 S_SWIPE_BACKGROUND_RIGHT = roi_front(1093,148)→roi_back(397,140)；旧 flow 用 (1100,380)→(250,380) → 改为真实几何 swipe(1103,158 → 407,150)
- 修复：fire() 重试：点战斗图标后若仍在章节地图（没进战斗）旧 flow 直接 wait 25s 会失败 → 增加最多 4 次有界重试（仍在 章节_固定阵容 就再 click_target 点怪）
- 修复：进战斗 marker：旧 flow 只认 I_BATTLE_INFO；tasks/探索 真机验证链路用 战斗_自动按钮 → 改为 `I_BATTLE_INFO or 战斗_自动按钮`（OR 保持两条路径）
- 修复：MAIN 场景缺分支：补 I_UI_CONFIRM / I_UI_CONFIRM_SAMLL 弹窗点掉与 I_TREASURE_BOX_CLICK 宝箱（上游 _handle_treasure_box / 弹窗清理）
- 修复：清除重复条件 `or appear('章节_固定阵容') or appear('章节_固定阵容')`
- 仍存在（blocker/未迁移）：章节固定第二十八章、UP 定向、auto_rotate、滑屏尽头（帧差分）仍为 blocker（未在本轮改动）
- 校验：`oasd task lint --tasks-dir tasks --task Exploration` => errors=0
