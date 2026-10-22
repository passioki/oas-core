# 个人突破（统一任务模板 M1b）

阴阳师 2.8.84 · MuMu Android 12（vmindex=0，adb `127.0.0.1:16384`）· 设计分辨率 **1280×720**

## 目录

```
tasks/个人突破/
├─ meta.json     # §22.34.2 契约：entry/params/exits（含 no_tickets:resource）/capabilities/...
├─ flow.json     # DSL；票不足出口资源耗尽；目标不可打另有 no_target 出口
├─ assets.json   # 7 个 template + 1 个 OCR 资产（突破券数字）+ design_resolution
└─ images/a001..a008.png
```

## 运行（需要 OCR 模型：入口的突破券判定用 OCR）

```powershell
cd D:\OAS-dev\oas-core
$env:ADB_PATH='D:\OAS-dev\backend\toolkit\Lib\site-packages\adbutils\binaries\adb.exe'
$m='D:\OAS-dev\backend\toolkit\Lib\site-packages\rapidocr\models\PP-OCRv6_rec_small.onnx'
$d='D:\OAS-dev\backend\toolkit\Lib\site-packages\rapidocr\models\PP-OCRv6_det_small.onnx'
.\target\debug\oasd.exe task lint --tasks-dir tasks --task 个人突破   # => lint task=个人突破 errors=0
.\target\debug\oasd.exe run --task tasks\个人突破 --seed 7 `
  --clicks-out D:\OAS-dev\_verify\n42\个人突破.clicks.jsonl `
  --shots-dir  D:\OAS-dev\_verify\n42\shots_个人突破 `
  --model $m --det-model $d
```

默认挑战 1 次；`--param max_attacks=N` 可在同一次运行里连续挑战 N 次（目标被攻破后由备用目标兜底）。

## 流程（flow.json）

| # | op | 说明 | 守卫 |
|---|----|------|------|
| 1 | click_point (680,152) | 庭院「探索」灯笼 | requires `庭院_探索灯笼` / expect `世界_章节栏` |
| 2 | click_point (297,647) | 探索大世界底部「结界突破」 | requires `探索_结界突破图标` / expect `结界突破_个人页签` |
| 3 | if `ocr_digit('突破券') != 0` | **票量判定**（29/30 → 29） | OCR 资产 `突破券`(roi 1160,6,112,38) |
| 3a | else 分支 | 票不足：log → 关闭页面 → 回庭院 → `finish(resource_empty)`（模板契约名 `no_tickets`，result=resource） | 两条返回点击都有守卫 |
| 4 | loop×`param('max_attacks')` | 每轮：点目标 A(右上 1010,190)，`if appear('进攻按钮')` → 攻击；否则点目标 B(左中 280,390) 兜底；两处都不可打 → `finish(no_target)` | 每次攻击：click_target「进攻」→ expect 战斗准备 → 点「准备」→ wait_until 战斗 UI → 循环点继续直到回到个人突破页 |
| 5 | click_point (1208,128) | 右上红 X 关闭个人突破 | expect `世界_章节栏` |
| 6 | click_point (56,58) | 大世界返回 → 庭院 | expect `庭院_探索灯笼` |
| 7 | finish done | | |

- 进攻按钮用 `click_target`（全帧模板搜索）：目标卡片在九宫格里的位置不同，弹窗会贴着卡片出现，
  固定坐标不可靠（实测右上目标的弹窗「进攻」在 (1052,404)，左目标的在 (387,390)）。
- OCR 只用于「票是否为空」；模板链路不需要 OCR。
- 结算阶段用「点 (640,690) + `结界突破_个人页签` 重新出现」判断结束，对胜负两种结算都成立。

## 素材（assets.json）

| id | 来源帧 | roi | threshold | 用途 |
|----|--------|-----|-----------|------|
| 庭院_探索灯笼 | t07_home | 638,105,88,85 | 0.88 | 庭院起点守卫 |
| 世界_章节栏 | t02_worldA | 1070,165,160,48 | 0.80 | 探索大世界守卫 |
| 探索_结界突破图标 | t02_worldA | 266,626,62,46 | 0.88 | 底部栏「结界突破」守卫 |
| 结界突破_个人页签 | r02_jjbk | 1185,255,62,85 | 0.90 | 个人突破页守卫（页内判定/结算判定） |
| 进攻按钮 | r03_select | 340,358,98,66 | 0.88 | click_target 点「进攻」 |
| 战斗_准备按钮 | r04_b1 | 1118,542,110,105 | 0.88 | 战斗准备页守卫/点「准备」 |
| 结界突破_返回X | r02_jjbk | 1182,100,58,58 | 0.88 | 关闭个人突破页 |
| 战斗_自动按钮 | r05_c2 | 20,645,110,58 | 0.85 | 确认已进入战斗（`wait_until`） |
| 突破券（OCR） | — | 1160,6,112,38 | — | `ocr_digit('突破券')` 读券数 |

阈值同样经 `_verify/n42/validate_templates.py` 的 12 状态全帧 NCC 矩阵标定。

## 前置条件 / 已知限制

1. **起始画面必须是庭院**；**票不足出口**已声明并实现（`resource_empty` → outcome=Done、
   `result=resource`），但因本机有 29+ 张突破券，真实「票=0」场景尚未实测——见
   `_verify/n42/REPORT.md` 的「分支验证」：用测试副本把条件强制为 false，验证了 else 分支
   （退回庭院 + finish resource_empty + outcome=Done）确实可执行。
2. **OCR 依赖**：跑本任务必须带 `--model/--det-model`（当前 debug 二进制已带 ocr feature）。
   模型路径见上面的运行命令（来自旧仓 toolkit 的 rapidocr 模型）。
3. **胜负**：当前账号可用阵容只有「默认分组 / 通用阵容」(1×60 + 5 素材)，对 54~55 级玩家队伍
   **通常会战败**（本实机证据：`shots_个人突破/06_结算_失败.png`）。flow 只保证「完成一次真实挑战 +
   结算 + 返回」，不判胜负；战败不产生「破」印章，因此同一目标可重复挑战，任务可重复运行。
   想要胜利需要先在游戏里配置更好的「预设/式神录」阵容（flow 未替玩家改阵容）。
4. **目标兜底**：默认打右上角目标，若该目标已被攻破（九宫格出现「破」印章）则改打左中目标；
   两处都被攻破时以 `no_target`（blocked）结束。点「刷新」重置九宫格未纳入 flow（有 CD/确认弹窗）。
5. **票数会缓慢恢复**；每次挑战消耗 1 张突破券。
6. 未处理「呱太入侵」活动变体（旧仓 RealmRaid 有专门分支）；当前账号/时段未出现该活动。
