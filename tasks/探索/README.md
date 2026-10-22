# 探索（统一任务模板 M1b）

阴阳师 2.8.84 · MuMu Android 12（vmindex=0，adb `127.0.0.1:16384`）· 设计分辨率 **1280×720**

## 目录

```
tasks/探索/
├─ meta.json     # §22.34.2 契约：entry/params/exits/capabilities/resources/schedule_hint
├─ flow.json     # DSL（仅 8 种现有 op；finish 只用引擎支持的 exit 名）
├─ assets.json   # 8 个 template + design_resolution=[1280,720]
└─ images/a001..a008.png
```

## 运行

```powershell
cd D:\OAS-dev\oas-core
$env:ADB_PATH='D:\OAS-dev\backend\toolkit\Lib\site-packages\adbutils\binaries\adb.exe'
.\target\debug\oasd.exe task lint --tasks-dir tasks --task 探索   # => lint task=探索 errors=0
.\target\debug\oasd.exe run --task tasks\探索 --seed 7 `
  --clicks-out D:\OAS-dev\_verify\n42\探索.clicks.jsonl `
  --shots-dir  D:\OAS-dev\_verify\n42\shots_探索
```

默认 1 场战斗；要打多场可加 `--param max_battles=2`（同一次进入章节地图内连打，妖怪会出现在同一张图上）。

## 流程（flow.json）

| # | op | 说明 | 守卫 |
|---|----|------|------|
| 1 | click_point (680,152) | 庭院「探索」灯笼 | requires `庭院_探索灯笼` / expect `世界_章节栏` |
| 2 | click_point (1140,462) | 章节列表「第二十八章」行 | requires `章节_第二十八章` / expect `入口_探索按钮` |
| 3 | click_point (1115,630) | 入口页「探索」按钮 | requires `入口_探索按钮` / expect `章节_固定阵容` |
| 4 | click_point (640,360) | 点空地清剧情对话（无对话无副作用） | requires `章节_固定阵容` |
| 5 | loop×`param('max_battles')` | 每轮：click_target 章节地图上的妖怪 → wait_until 战斗 UI(`战斗_自动按钮`) → 循环点「点击屏幕继续」直到回到章节地图 | click_target requires/expect：`章节_固定阵容` 消失即已开战 |
| 6 | click_point (53,32) | 章节地图左上返回 | expect 出现「确认退出」对话框 |
| 7 | click_point (780,401) | 「确认」退出章节 | expect `入口_探索按钮` |
| 8 | click_point (53,32) | 入口页返回 → 探索大世界 | expect `入口_探索按钮` 消失 |
| 9 | click_point (53,32) | 大世界返回 → 庭院（有加载过场） | expect `庭院_探索灯笼` |
| 10 | finish done | | |

- 点击点用 `click_point`（固定 1280×720 坐标）+ `requires`/`expect` 守卫；只有「地图上的妖怪」用
  `click_target`（模板全帧搜索 §22.20，妖怪位置会变也能找到）。
- 战斗为自动战斗（游戏内「自动挑战」开启时点中妖怪即开战），不判断胜负；当前账号阵容打 40 级章节怪稳胜。

## 素材（assets.json）

| id | 来源帧 | roi | threshold | 用途 |
|----|--------|-----|-----------|------|
| 庭院_探索灯笼 | t07_home | 638,105,88,85 | 0.88 | 庭院起点守卫/点击点守卫 |
| 世界_章节栏 | t02_worldA | 1070,165,160,48 | 0.80 | 探索大世界守卫（半透明面板，阈值经 12 状态矩阵标定） |
| 章节_第二十八章 | t02_worldA | 1067,447,142,42 | 0.86 | 确保列表当前停在该章（防止点错行） |
| 入口_探索按钮 | t03_entrance | 1058,588,130,102 | 0.88 | 章节入口页守卫/按钮 |
| 章节_固定阵容 | t04_chapter | 884,642,175,46 | 0.90 | 章节地图守卫（战斗前/后判断） |
| 章节_战斗图标 | t04_chapter | 912,242,64,64 | 0.85 | click_target 找妖怪 |
| 退出确认_确认 | s03_back1 | 692,373,176,57 | 0.90 | 退出章节确认对话框 |
| 战斗_自动按钮 | t05_v2 | 20,645,110,58 | 0.80 | 确认已进入战斗（`wait_until`） |

阈值不是拍脑袋：`_verify/n42/validate_templates.py` 用 OpenCV `TM_CCOEFF_NORMED`（与引擎
NCC 等价）对 12 个界面状态算全帧最高分，保证「本状态高分、其它状态低分」，再用
`oasd task lint` + 实机 run 复核。

## 前置条件 / 已知限制

1. **起始画面必须是庭院**（flow 第 1 步 requires `庭院_探索灯笼` 会在别的界面响亮失败）。
2. **当前选中章节必须是第二十八章**：从庭院进「探索」后章节列表会滚动到当前章节，
   本 flow 固定点该行 (1140,462) 并用 `章节_第二十八章` 守卫。换章（例如在列表里点了别的章）
   后需把坐标改到新行或用 OCR 选章。
3. **「自动挑战」必须开启**（本机已开）：关闭时点妖怪只会弹「挑战」确认，`expect 章节_固定阵容 消失`
   会超时并响亮报错（不会静默误点）。
4. 每场战斗消耗体力 3；体力不足时点妖怪无效 → `expect vanish` 超时 → Failed（meta.exits 声明了
   `failed: retryable`；模板契约的 `no_resource` 结果类暂无对应引擎出口，如需要可加 OCR 体力判定）。
5. 章节地图每次进入妖怪重新出现，因此任务可重复运行；每轮会杀掉一只妖怪。
6. 战斗结算界面文案/位置随版本可能变；本实现用「点 (640,690) + 用 `章节_固定阵容` 出现判断结束」，
   对「胜利 / 奖励 / 剧情」多个继续画面都通用。
7. 仅使用引擎已实现的 op（click/click_point/click_target/wait/wait_until/loop/if/break/log/finish），
   不依赖引擎内部代码；引擎升级（M0 模板契约）后 `oasd task lint` 通过、实机复跑通过。
