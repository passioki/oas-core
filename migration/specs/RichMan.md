# RichMan — 行为矩阵（迁移规格）

源：`runhey/OnmyojiAutoScript@dev` · `tasks/RichMan/`（Weekly Task）

## 源文件

| 文件 | 作用 |
|------|------|
| tasks/RichMan/script_task.py | run：4 个 execute_* 顺序调用 |
| tasks/RichMan/config.py | 100+ 行配置：6 大块 + 商城 5 子商店 + 寄售/密卷/契灵 |
| tasks/RichMan/assets.py | 120 项资产（模板/区域/OCR） |
| tasks/RichMan/thousand_things.py | execute_tt / tt_* 购买 / tt_earn_money / tt_get_reward |
| tasks/RichMan/shrine.py | execute_shrine / shrine_*（黑蛋/五星/四星） |
| tasks/RichMan/guild.py | execute_guild / _guild_*（蓝票/黑碎/皮肤券） |
| tasks/RichMan/mall/mall.py | execute_mall：寄售→密卷→契灵→特殊→荣誉→友情→勋章→魅力→back_mall |
| tasks/RichMan/mall/navbar.py | MallNavbar：_enter_* / back_mall / mall_resource / mall_check_money |
| tasks/RichMan/mall/*.py | special / honor / friendship_points / medal / charisma / consignment / scales / bondlings |
| tasks/Component/Buy/buy.py | buy_one / buy_more / buy_one_or_more / buy_check_money |
| tasks/GameUi/default_pages.py | page_* 导航 |

## 行为矩阵（按旧仓模块）

| 模块 | 旧仓行为 | flow 覆盖 | 备注 |
|------|----------|-----------|------|
| TT | enable 开关；goto travel；进宝箱；earn_money；买蓝票/黑碎/AP；领奖；exit | ✅ 参数开关 + 全链 | 金额门槛 ocr_digit；数量拉满 |
| Shrine | enable；goto summon；进神龛；黑蛋/五星/四星（金币门槛 1500/1200/400） | ✅ | 同上 |
| Guild | enable；goto guild；进商店；蓝票/黑碎/皮肤券 | ✅ | 剩余数量动态 ROI 未迁移 |
| Mall | **总是** goto mall；各子商店由 enable 控制 | ✅ 导航 + 各商店 | |
| 寄售屋 | 勾玉门槛 100；剩余次数；买兑换券 | ✅ 近似 | 剩余次数 OCR 简化 |
| 密卷屋 | 朴素御魂/首领御魂（boss+位置）/海国御魂 | ◑ orochi/sea 完整，demon 选择简化 | 单价/剩余 OCR 简化 |
| 契灵 | 4 类商品（石头/御魂/高级盘/中级盘） | ✅ 拉满 + 余额门槛 | 剩余数量未用 |
| 特殊 | 御灵/中级盘/低级盘 + 下滑 | ◑ 仅御灵 | 中级/低级盘未迁移 |
| 荣誉 | 蓝票/黑碎 | ✅ | |
| 友情点 | 白蛋(单买)/红蛋/破碎符 | ✅ | |
| 勋章 | 8 类商品 | ✅（数量拉满） | |
| 魅力 | 黑碎/蓝票 | ✅ | |
| back_mall | goto_page(page_mall) | ✅ 循环回 CHECK_MALL | |
| 结束 | 停在商城页 | ✅ finish done | |

## 素材

见 README；RichMan 120 项 + Component/Buy 11 项 + GameUi/GlobalGame 导航奖励资产。

## 出口

`done` / `failed`。

## 缺口 / TODO

1. 旧仓依赖运行时**动态 ROI**（`O_GUILD_REMAIN.roi = 图像 roi_front ± 偏移`、`GeneralBuff.get_area` 同类）；
   DSL 无动态 ROI / 锚点定位 → 功勋商店「本周剩余数量」判断、部分购买数量计算未迁移。
2. 数量/单价/剩余为 `DigitCounter` / `Quantity` / `Single` 多值 OCR；`ocr_digit` 只取首个数字 → 购买数量改用「拉满」。
3. RuleSwipe 已导入为 `_FROM/_TO` region id，但 flow 用 `swipe` op + 推导坐标（S_GUILD_STORE / S_SP_DOWN）。
4. RuleList（如商城底部导航列表）无 DSL op → 导航用逐页模板点击实现。
5. 特殊屋中级盘/低级盘（需要滑动找商品）未迁移。
6. 未真机验证。
