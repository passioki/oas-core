# Pets — 行为矩阵（迁移规格）

源：`runhey/OnmyojiAutoScript@dev` · `tasks/Pets/`（只读 checkout `D:\OAS-dev\_verify\remote-oas`）

## 源文件

| 文件 | 作用 |
|------|------|
| tasks/Pets/script_task.py | ScriptTask.run / _feed / _play |
| tasks/Pets/config.py | PetsConfig.pets_feast（默认 True） |
| tasks/Pets/assets.py | I_PET_* 模板、O_PET_FEED_AP / O_PET_PLAY_GOLD OCR |
| tasks/GameUi/default_pages.py | page_pet 定义与 page_main→page_pet 连接 |
| tasks/GameUi/navigator.py | goto_page / ui_click / appear_then_click 语义（只读参考） |

## 行为矩阵

| 旧仓行为 | 触发条件 | 新 flow 覆盖 | 备注 |
|----------|----------|--------------|------|
| goto_page(page_main) | run 开始 | 由 meta.entry（`I_CHECK_MAIN`）表达 | 起始画面必须是庭院 |
| 循环找宠物界面：`I_PET_FEAST` | 每次循环 | ✅ loop×8 首查 feast | |
| `appear_then_click(I_PET_HOUSE)` | feast 未出现 | ✅ loop 内 click | requires=appear |
| `appear_then_click(I_PET_CLAW)` | feast 未出现 | ✅ loop 内 click | |
| `_feed()`：`ui_click(I_PET_FEAST, I_PET_FEED)` | pets_feast | ✅ click FEAST expect FEED | |
| `O_PET_FEED_AP.ocr == 0` → 已投喂返回 | 体力=0 | ✅ if ocr_digit(...)==0 | 等价 |
| `ui_click(I_PET_FEED, I_PET_SKIP)` | 体力>0 | ✅ click FEED expect SKIP | |
| `wait_until_disappear(I_PET_SKIP)` | 动画结束 | ✅ loop 30×1s 有界等待 | 旧实现无超时 |
| `ui_click(I_PET_EXIT, I_CHECK_MAIN)` | 结束 | ✅ click EXIT expect HOME | |
| `_play()` 玩耍 | pets_happy（已注释停用） | ❌ 不迁移 | 上游代码已注释 |

## 素材使用

见 README 素材表；全部来自全局素材库（`legacy_Pets_*`），任务本地 `assets.json` 为空。
旧仓 `I_PET_HAPPY/I_PET_PLAY/O_PET_PLAY_GOLD` 保留未用。

## 出口

`done`(success) / `failed`(retryable)。

## 缺口 / TODO

1. 真机/回放未验证（无游戏会话）。
2. 入口路径按旧仓 `default_pages.py` 页图手写；若客户端主界面布局变化需重新标定 `I_PET_HOUSE` 匹配。
3. OCR 需要 `--model/--det-model`。
