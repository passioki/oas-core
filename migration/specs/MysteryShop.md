# MysteryShop（神秘商店）迁移规格

- 源仓库：`runhey/OnmyojiAutoScript@dev` · commit `8b4dc669b47838f42d283f14aa394844c1963c86` · 只读副本 `D:/OAS-dev/_verify/remote-oas`
- 目标：`tasks/MysteryShop/`（统一任务模板 + 全局 `legacy_*` 素材）
- 迁移结论：**🔄** —— 行为矩阵见 §3，缺口见 §5

## 1. 读取的源文件

- `tasks/MysteryShop/script_task.py`
- `tasks/MysteryShop/config.py`
- `tasks/MysteryShop/assets.py`
- `tasks/RichMan/mall/friendship_points.py (buy_mall_one, imported)`
- `tasks/Component/GeneralInvite/general_invite.py (share/invite, imported)`

## 2. 入口 / 出口

- 入口：`appear('庭院_探索灯笼')`；flow 首段 `ensure_main()` 回庭院。
- 出口：
  - `done` → result `success`
  - `failed` → result `retryable`

## 3. 行为矩阵

| # | 行为 | 远程实现（源码位置） | 新 DSL 映射 | 状态 |
|---|---|---|---|---|
| 1 | 进入神秘商店 | script_task.py:28-30 goto_page(page_mall); ui_click(I_ME_ENTER→I_MS_SHARE) | main→I_MAIN_GOTO_MALL→loop click I_ME_ENTER 直到 I_MS_SHARE | 覆盖 |
| 2 | 分享邀请 | script_task.py:107-176 share + invite_friend(1..5) | — | 缺口：GeneralInvite 流程未迁移 |
| 3 | 四类商品购买 | script_task.py:84-105 run_shop + RichMan buy_mall_one | 4 × param 分支 → if 购买按钮出现→click→确认；售罄标记出现即跳过 | 部分（无价格上限/多数量） |
| 4 | 切换好友商店 | script_task.py:44-80 next_one（O_MS_FRIEND 变化） | ocr_to_var(O_MS_FRIEND) + loop click I_MS_NEXT，最多 5 位 | 部分 |
| 5 | 商店奖励 | script_task.py:177-194 O_MS_RECORDS ≥3/5/10 → I_MS_REWARD_* | ocr_digit(O_MS_RECORDS) 三个阈值分支 | 覆盖 |
| 6 | 回庭院 | script_task.py:39 back_mall | ensure_main | 覆盖 |
| 7 | 开业日/next_time | script_task.py:24-27,196-222 | — | 缺口：无时钟原语 → scheduler windows |

## 4. 参数映射

| key | type | default | 说明 |
|---|---|---|---|
| `mystery_amulet` | bool | `False` | 远程配置/迁移决策 |
| `black_daruma_scrap` | bool | `False` | 远程配置/迁移决策 |
| `shop_kaiko_3` | bool | `False` | 远程配置/迁移决策 |
| `shop_kaiko_4` | bool | `False` | 远程配置/迁移决策 |

## 5. 缺口 / TODO

- 分享（share_config：邀请 1~5 位好友）需要 GeneralInvite 完整找好友流程，未迁移 → flow 跳过。
- buy_mall_one 的 max_price 单价读取（O_MALL_RESOURCE_5 等 RichMan mall 资源 OCR + 弹窗价格上限判断）与多数量窗口(more_window)未迁移；当前只做「点购买→点确认」的一键购买。
- 好友切换 O_MS_FRIEND 变化检测用 ocr_to_var + 固定切换次数近似，未严格比对上一帧。
- 开业日（周三/周六）判断与 next_time 排期无时钟原语 → 交给 scheduler.json 的 windows。

## 6. 素材契约与通用模式

- 导入器 v3：模板 `roi` = remote `roiBack`，无 `action_roi` ⇒ `click` 落在匹配到的模板矩形（= 旧引擎行为）；`RuleClick` → `kind=region`；`RuleOcr` → `ocr_lines`；`RuleSwipe` → `_FROM`/`_TO` 两个 region；Component 素材 `legacy_Component_<Module>_<CONST>`。
- P-NAV / P-BATTLE v2 / P-RETURN / OCR 语义同批一 README。

## 7. 证据

- oasd task lint 绿；覆盖：商店入口→好友商店遍历→四类商品购买(含购买确认)→记录奖励领取→回庭院；分享邀请与价格上限未覆盖。未真机/回放验证。
- 未真机/未回放：无设备、无对应场景回放帧；T4 素材引用全部解析到全局 `assets/assets.json`。
