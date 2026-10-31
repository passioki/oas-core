# MysteryShop（神秘商店）

> 来源：`runhey/OnmyojiAutoScript@dev`（只读 checkout `D:/OAS-dev/_verify/remote-oas`，commit `8b4dc669b478`）。
> 迁移方式：行为抽象 + 既有 oas-flow DSL（ENGINE-DESIGN §22），无 Python 代码拷贝。

## 读取的源文件

- `tasks/MysteryShop/script_task.py`
- `tasks/MysteryShop/config.py`
- `tasks/MysteryShop/assets.py`
- `tasks/RichMan/mall/friendship_points.py (buy_mall_one, imported)`
- `tasks/Component/GeneralInvite/general_invite.py (share/invite, imported)`

## 行为摘要

庭院→商店→神秘商店：遍历好友商店，按开关购买神秘符咒/黑达摩碎片/太鼓3/太鼓4；按记录数领取 3/5/10 次奖励；回庭院。

## 参数 / 出口

- `mystery_amulet` (bool) default=False
- `black_daruma_scrap` (bool) default=False
- `shop_kaiko_3` (bool) default=False
- `shop_kaiko_4` (bool) default=False

- exit `done` → `success`
- exit `failed` → `retryable`

## 行为矩阵（详见 migration/specs/MysteryShop.md）

- 进入神秘商店：覆盖
- 分享邀请：缺口：GeneralInvite 流程未迁移
- 四类商品购买：部分（无价格上限/多数量）
- 切换好友商店：部分
- 商店奖励：覆盖
- 回庭院：覆盖
- 开业日/next_time：缺口：无时钟原语 → scheduler windows

## 已知缺口 / TODO

- 分享（share_config：邀请 1~5 位好友）需要 GeneralInvite 完整找好友流程，未迁移 → flow 跳过。
- buy_mall_one 的 max_price 单价读取（O_MALL_RESOURCE_5 等 RichMan mall 资源 OCR + 弹窗价格上限判断）与多数量窗口(more_window)未迁移；当前只做「点购买→点确认」的一键购买。
- 好友切换 O_MS_FRIEND 变化检测用 ocr_to_var + 固定切换次数近似，未严格比对上一帧。
- 开业日（周三/周六）判断与 next_time 排期无时钟原语 → 交给 scheduler.json 的 windows。

## 复用的通用模式

- **P-NAV**：`legacy_GameUi_I_CHECK_MAIN` → `click legacy_GameUi_I_*_GOTO_*`（模板点击 = 匹配到的模板矩形）→ 目标页 marker。
- **P-BATTLE v2**：`Component_GeneralBattle_I_PREPARE_HIGHLIGHT` → 点准备 → 等 `I_WIN/I_FALSE/I_REWARD/I_REWARD_GOLD` → 逐一点掉胜负横幅与奖励 → 等任务页 marker。
- **P-RETURN**：`GlobalGame_I_UI_BACK_*` / `GameUi_I_BACK_YOLLOW` / 坐标兜底 → `I_CHECK_MAIN`。
- **OCR**：`RuleOcr` 已导入为 `ocr_lines`；计数 `ocr_digit`，条件 `text_visible`，按行点击 `click_target {kind:ocr_text, sel|roi, text}`。运行需 `--model/--det-model`。

## 证据

- `oasd task lint --tasks-dir tasks --task MysteryShop` → 0 errors；状态见 `migration/status/MysteryShop.json`。
- 未真机 / 未回放验证（迁移会话无可用设备与对应场景回放）。
