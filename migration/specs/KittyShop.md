# KittyShop — 行为矩阵（迁移规格）

源：`runhey/OnmyojiAutoScript@dev` · `tasks/KittyShop/`（Activity Task）

## 源文件

| 文件 | 作用 |
|------|------|
| tasks/KittyShop/script_task.py | run / _run / _select_kitty / _main_select_kitty |
| tasks/KittyShop/config.py | kitty_attempts、kitty_quit_when_finished |
| tasks/KittyShop/assets.py | 模板 + RuleClick + RuleSwipe |
| tasks/ActivityShikigami/assets.py | I_SHI 活动入口 |
| tasks/GameUi/default_pages.py | page_main / 页面跳转参考 |

## 行为矩阵

| 旧仓行为 | 新 flow 覆盖 | 备注 |
|----------|--------------|------|
| goto_page(page_main) | ✅ meta.entry 庭院 | |
| 入口循环 START_FARMING/GO1/I_SHI | ✅ loop 30 | |
| attempts 次数 | ✅ param('attempts') 默认 1 | |
| 选猫：点 START_FARMING | ✅ | expect START_ENSURE |
| 选猫：I_UNSELECTED 循环，轮换 C_SELECT + swipe | ◑ 每轮顺序点 4 列 + 左滑 | 计时节奏不同，需真机 |
| 3 次 SelectFailed 重试 | ❌ | 有界循环替代 |
| 点 START_ENSURE 开始 | ✅ | |
| 经营循环：SHARE 结束 | ✅ loop 240 | |
| quit_when_finished：确认/完成 | ✅ 参数分支 | GlobalGame 确认框 |
| GIFT/ADD/FLAG1→BUSY 卡 | ✅ | |
| 结算后 FLAG1 回 START_FARMING | ✅ loop 15 | |
| 黄返回庭院 | ✅ | |

## 素材

见 README；`S_SELECTION`（RuleSwipe）不导入 → flow 用推导 swipe 坐标。

## 出口

`done` / `failed`。

## 缺口 / TODO

1. 选猫点击节奏/滑动时机需真机确认；SelectFailed 重试未迁移。
2. RuleSwipe 未导入全局素材库（importer 只处理 Image/Click/LongClick/Ocr）。
3. 未真机验证。
