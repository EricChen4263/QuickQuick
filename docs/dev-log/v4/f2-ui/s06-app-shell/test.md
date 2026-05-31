# V4/F2/S06 app-shell 测试报告

日期：2026-05-31  
执行者：tester agent（动态证伪）  
被测版本：main 分支（未提交工作树含 App.tsx / tauri.conf.json / vite.config.ts 改动）

---

## 开工/结束 git 快照（逐行一致，工作树无污染）

```
 M package.json
 M pnpm-lock.yaml
 M src-tauri/tauri.conf.json
 M src/App.tsx
 M vite.config.ts
?? docs/dev-log/v4/f2-ui/
?? src/app-shell.test.tsx
?? src/test-setup.ts
```

---

## 档位一：命中校验

命令：
- `pnpm test app-shell --reporter=verbose`
- `pnpm test`

app-shell 结论：
```
✓ app-shell: 左侧边栏渲染三个一级入口（剪贴板/翻译/设置）
✓ app-shell: 默认激活剪贴板页，page-clipboard 可见、page-translate 不可见
✓ app-shell: 点击翻译后 page-translate 变为可见、page-clipboard 隐藏
✓ app-shell: 点击设置后 page-settings 变为可见
✓ app-shell: 当前选中项有 aria-current 属性，默认选中剪贴板
✓ app-shell: 点击翻译后翻译入口获得 aria-current、剪贴板入口失去
Test Files  1 passed (1)
Tests  6 passed (6)
```

全量结论：
```
Test Files  14 passed (14)
Tests  112 passed (112)
```

判定：命中 N=6，无假绿，无因 jsdom 引入而红的既有测试，全量 112 全绿。通过。

---

## 档位二：变异 sanity

备份命令：`cp src/App.tsx /tmp/App.tsx.bak`  
复原命令：`cp /tmp/App.tsx.bak src/App.tsx`（严禁 git checkout）

### 变异一：page-translate display 改成恒 "block"

改动：第 94 行 `display: activeTop === "translate" ? "block" : "none"` → `display: "block"`

跑 app-shell 结果：
```
FAIL  app-shell: 默认激活剪贴板页，page-clipboard 可见、page-translate 不可见
      Error: expect(element).not.toBeVisible() — 元素实际可见（display:block）
Test Files  1 failed (1)
Tests  5 passed | 1 failed
```

结论：如期变红。translate 切换逻辑真实被测，非恒真。

从备份复原：完成，`grep` 确认第 94 行恢复条件表达式。

### 变异二：entries 过滤掉 translate 入口

改动：第 69 行 `const entries = topLevelEntries()` → `const entries = topLevelEntries().filter((e) => e !== "translate")`

跑 app-shell 结果：
```
FAIL  app-shell: 左侧边栏渲染三个一级入口（剪贴板/翻译/设置）
FAIL  app-shell: 点击翻译后 page-translate 变为可见、page-clipboard 隐藏
FAIL  app-shell: 点击翻译后翻译入口获得 aria-current、剪贴板入口失去
Test Files  1 failed (1)
Tests  3 passed | 3 failed
```

结论：如期变红。入口存在性校验真实有效，非旁路。

从备份复原：完成，`grep` 确认第 69 行恢复 `topLevelEntries()`。

结束时 git 快照与开工逐行一致，工作树无新增/丢失。

---

## 档位三：边界探测

临时测试文件 `src/boundary-probe.test.tsx`（跑完已删除，工作树无残留）

探测内容：
1. 点同一入口翻译两次，翻译页仍可见（重复点击幂等性）
2. 翻译再切回剪贴板，clipboard 重新可见（双向切换）
3. settings→clipboard→settings 循环切换，settings 仍可见（多步循环）

结果：
```
✓ 点同一入口翻译两次，翻译页仍可见
✓ 翻译再切回剪贴板，clipboard 重新可见
✓ settings->clipboard->settings 循环切换，settings 仍可见
Test Files  1 passed (1)
Tests  3 passed (3)
```

jest-dom matchers（toBeVisible / toHaveAttribute / toBeInTheDocument）全部生效，无"matcher 未定义"报错。

边界均优雅处理，无 panic、无静默错。

---

## 覆盖缺口

当前测试未覆盖：
- Esc 键触发 `getCurrentWindow().hide()` 的路径（可考虑补充）
- route 事件 `listen` mock 实际调用了回调（route 事件→自动切换页面的逻辑）

以上属覆盖缺口，但未发现真实缺陷，不构成打回条件。

---

## 门禁结论

**放行**

- 命中校验：6/6 通过，14 文件 / 112 测试全绿，无假绿
- 变异 sanity：2 处改坏均如期变红，测试有真实判别力，非恒真/旁路
- 边界探测：3 项边界全通，实现行为正确
- 工作树还原：开工/结束 git 快照逐行一致，无业务代码改动残留
