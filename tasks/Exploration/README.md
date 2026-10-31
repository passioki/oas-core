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
