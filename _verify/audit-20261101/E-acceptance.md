# E — 2026-11-01 验收修复（点击分布 / 反选 / 真实测试）

## 1. 点击分布

- 删除画布左上角的 `clicks=N distinct=K` 指示条和画布图例：`drawClickStats()` 已删除，`drawClickOverlay()` 只画热力/散点。
- 侧栏 `#click-stats` / `#clicks-legend` 保留。
- 浏览器实测：聚合 1 条运行（clicks=8 distinct=8）后画布 `fillText` 为空，侧栏统计正常。

## 2. 操作区域反选

- `actionShapeFromGeometry()` 统一生成 rect/polygon 并携带 `invert`；`currentActionShape()` 使用它。
- `loadActionShapeForEdit()` / `editAssetActionRoi()` 恢复矩形与多边形的 invert；素材行“操作区”保存使用 `currentActionShape()`。
- 画布：
  - 矩形/多边形操作区统一橙色虚线 `setLineDash([8,5])`；
  - 反选有效区=整屏冷色 `rgba(49,208,170,.10)`，原选中区=暖橙 `rgba(255,159,64,.26)`。
- 浏览器像素采样：
  - 反选有效区 `[49,206,167]`，原选中区 `[207,168,87]`；
  - 矩形/多边形 8 项颜色检查全部通过。
- 文件：`apps/oasd/ui/app.js`（`actionShapeFromGeometry` / `loadActionShapeForEdit` / `drawActionShapeOverlay` / `editAssetActionRoi`）。

## 3. 真实点击/演练管理按钮

- header `#btn-live-preview` 旁新增 `#btn-test-mode`，默认“素材测试：演练”；实时按钮仍在。
- 真实模式：
  - `realAssetTestGuard()` 要求实时预览开启；
  - 点击 → `POST /input/tap {x,y}`；
  - 拖动 → `POST /input/swipe {from:[x,y],to:[x,y],ms}`；
  - 切换真实模式和每批手势发送前都有 `window.confirm`；
  - 结果显示 `device/gesture`，明确“真实已点击/已滑动”；
  - 取消则“已取消，未发送设备请求”。
- 默认演练；刷新回到演练（安全默认）。
- `#run-dry`（任务运行）保持独立，不伪造统一。
- 测试：`ui_pure_test` 108→179 checks，覆盖 rect/polygon invert、payload、真实/演练分支、无实时拒绝、确认文案、DOM 死 id=0。

## 4. 真实设备

设备：

```text
adb devices = emulator-5554 device
/state = 1280x720, device=emulator-5554, live=true
```

直接调用真实点击接口：

```text
POST /input/tap {x:640,y:40}
→ {"ok":true,"device":"emulator-5554","gesture":"tap","x":640,"y":40}
```

截图确认设备画面已响应；随后通过 `/input/tap` 关闭游戏弹窗，恢复到主界面。  
说明：真实 UI 手势路径已通过 payload/分支/浏览器验证；本次没有用 UI 真实模式再点一次游戏，避免误操作。真机 tap 能力已确认。

## 5. 测试

```text
cargo test --workspace = 393 passed / 0 failed
node ui_pure_test.js = 179 checks / 0 dead ids
task lint = 2/0
E2E = 36/36 PASS
```

## 6. 剩余风险

- 真实模式确认框阻塞期间实时画面可能变化，定位点可能相对确认后画面有延迟。
- collection 测试仍是 dry-run（服务端只返回 bbox，反选/多边形会不安全）。
- `asset_ui.js` 的 tooltip 仍可能写 dry-run；真实模式默认不持久化。
