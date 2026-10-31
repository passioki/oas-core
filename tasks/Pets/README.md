# Pets（宠物快速喂养）

阴阳师 2.8.84 · MuMu Android 12 · 设计分辨率 **1280×720**

## 来源（只读旧仓 `_verify/remote-oas`）

- `tasks/Pets/script_task.py`（87 行）— 入口循环、`_feed()`、`_play()`（后者上游已注释停用）
- `tasks/Pets/config.py` — `pets_feast=True`（快速喂养）
- `tasks/Pets/assets.py` — 8 个模板 + 2 个 OCR（喂食体力 / 玩耍金币）
- `tasks/GameUi/default_pages.py` — `page_pet = any_of(I_PET_CLAW, I_PET_FEAST)`；`page_main→page_pet` 用 `I_PET_HOUSE`

## 行为摘要（旧仓）

1. `goto_page(page_main)`（庭院）。
2. 循环：出现 `I_PET_FEAST` 即进入宠物界面；否则点 `I_PET_HOUSE`（主界面宠物小屋入口）或 `I_PET_CLAW`。
3. `pets_feast` 开启时 `_feed()`：点 `I_PET_FEAST` → 点 `I_PET_FEED` → `OCR O_PET_FEED_AP`，为 0 表示已投喂直接返回；
   否则点 `I_PET_FEED` 直到出现 `I_PET_SKIP`，等跳过按钮消失。
4. 点 `I_PET_EXIT` 回庭院（守卫 `I_CHECK_MAIN`）。

## 流程（flow.json）

| # | op | 说明 | 守卫 |
|---|----|------|------|
| 1 | loop×8 | 宠物界面入口循环：`I_PET_FEAST` 出现即停 / 否则点 `I_PET_HOUSE`、`I_PET_CLAW` | 每步 `requires appear` |
| 2 | appear | 断言已在宠物界面 | `I_PET_FEAST` |
| 3 | if `param('feed')` | 快速喂养分支 | 参数默认 true（=旧仓 `pets_feast=True`） |
| 3a | click `I_PET_FEAST` | 打开快速喂养页 | requires `I_PET_FEAST` / expect `I_PET_FEED` |
| 3b | if `ocr_digit('O_PET_FEED_AP') == 0` | 已投喂：只记日志，不再点喂食 | 与旧仓 `number == 0` 判断等价 |
| 3c | else | 点 `I_PET_FEED` → expect `I_PET_SKIP` → 循环等跳过消失 | requires/expect |
| 4 | click `I_PET_EXIT` | 回庭院 | requires `I_PET_EXIT` / expect `I_CHECK_MAIN` |
| 5 | finish done | | |

## 素材（全局 `assets/assets.json`，前缀 `legacy_Pets_`）

| id | 旧 const | 用途 |
|----|----------|------|
| legacy_Pets_I_PET_HOUSE | I_PET_HOUSE | 主界面入口 |
| legacy_Pets_I_PET_CLAW | I_PET_CLAW | 宠物页爪子入口 |
| legacy_Pets_I_PET_FEAST | I_PET_FEAST | 快速喂养按钮 / 界面守卫 |
| legacy_Pets_I_PET_FEED | I_PET_FEED | 喂食按钮 |
| legacy_Pets_I_PET_SKIP | I_PET_SKIP | 跳过（喂养动画） |
| legacy_Pets_I_PET_EXIT | I_PET_EXIT | 退出 |
| legacy_Pets_O_PET_FEED_AP | O_PET_FEED_AP | `ocr_digit` 读剩余喂食体力 |

`I_PET_HAPPY` / `I_PET_PLAY` / `O_PET_PLAY_GOLD` 上游 `_play()` 已注释，本流程不迁移；素材保留未用。

## 出口

- `done` → success：完成一次喂养检查并回庭院。
- `failed` → retryable：入口/喂养/退出任一守卫 timeout。

## 与旧仓的差异 / 已知缺口

1. **OCR 含义对齐**：旧仓 `O_PET_FEED_AP.ocr(image)` 是 Digit 模式单值；新引擎用 `ocr_digit('legacy_Pets_O_PET_FEED_AP')`（ocr_lines det+rec 后取首个数字），语义等价。
2. 旧仓 `wait_until_disappear(I_PET_SKIP)` 无超时；新引擎用「循环 30×1s 直到 SKIP 消失」的有界实现（不会无限等待）。
3. `_play()`（其乐融融/玩耍）上游已注释停用，未迁移。
4. 运行需要 OCR 模型（`--model/--det-model`）——入口循环和喂养判定都用到 OCR；未在真机验证。
5. 旧仓通过 `goto_page` 自动处理页面跳转/弹窗；本流程按旧仓页面图的固定路径手写（庭院→宠物小屋）。

## Lint

```powershell
cd D:\OAS-dev\oas-core
.\target\release\oasd.exe task lint --tasks-dir tasks --task Pets
```
