# 设置页面统一美化与重构方案

目前，项目的各个设置页（如通用设置、本地模型设置、麦克风设置等）由于是逐步添加的，样式和代码结构存在一些硬编码和不一致的情况。最近我们对 `LLM Service` 页面进行了卡片化和手风琴（折叠面板）的视觉升级，这可以作为整个设置页面的视觉基准。

为了实现“统一样式、统一可复用的格式”以及“更具美感”，我建议从组件抽取、样式重构和页面迁移三个层级进行改造。以下是详细方案供探讨：

## 1. 设计语言与交互基准统一 (Design System)

*   **布局与结构**：所有页面采用一致的 `<Section>`（模块区域）划分，不再只是简单的上下排列。每个区块有带图标的标题（Title）和辅助说明（Description）。
*   **卡片化视觉 (Card-based)**：将配置项或者核心功能放入具有浅色背景、圆角和微阴影的卡片中（类似 `FeatureCard`）。当配置较多时，采用折叠面板（`Accordion`）收纳。
*   **布局模式**：
    *   **行内布局 (Inline)**：标题与提示语在左，控制组件（开关 Switch、下拉框 Dropdown、短输入框等）对齐在右。
    *   **堆叠布局 (Stacked)**：整块内容的输入（如大段文字、复杂的图表、长输入框等），标题在上，控制组件在其下占满宽度。
*   **统一的颜色与间距体系**：完全依赖于现有的 CSS 变量（`var(--color-bg-secondary)` 等），确保暗黑/明亮模式下的完美切换。

## 2. 提取可复用的设置布局组件 (UI Components)

为了避免在各个 Tab 页面中不断手写 `<div className="settings-item">` 和重复的 DOM 结构，我们将抽象出一套专门用于设置页面的 React 组件：

*   **`SettingsSection` 组件**
    *   `title`: 区块标题
    *   `description` (可选): 区块的整体说明
    *   `icon` (可选): 标题左侧的 Lucide 图标
    *   `children`: 包含具体的 SettingsItem

*   **`SettingsItem` 组件**
    *   `title`: 设置项名称（例如“自动检查更新”）
    *   `hint` (可选): 设置项下方的灰色提示文本
    *   `layout`: `'horizontal'` (左右布局，推荐给开关/下拉) 或 `'vertical'` (上下布局，推荐给长输入框)
    *   `action` / `children`: 右侧操作区或下方的具体组件控件
    *   `withDivider`: 是否在底部显示分割线（如果是最后一个元素也可以自动隐藏）

*   **`SettingsCard` & `SettingsAccordion` 组件**
    *   从 `LLM Service` 把卡片和折叠面板的逻辑抽象出来，变成通用的 `<SettingsAccordion title="..." status="..." />` 工具组件。

## 3. 分阶段代码迁移

### 阶段一：建立统一套件
建立文件 `src/components/settings/SettingsLayout.tsx` 和全局性的 `SettingsUI.css`，将抽象好的 `SettingsSection`, `SettingsItem`, `SettingsAccordion` 放入其中。

### 阶段二：改造简单页面
优先挑选几个结构清晰的页面进行改造，验证套件的可用性：
*   **`SettingsGeneralTab` (通用设置)**：将语言、主题、字体、后台运行等改造成具有左右对其布局的 `SettingsItem`。
*   **`SettingsLocalTab` (本地模型)**：将各类参数提取为标准的设置项结构。

### 阶段三：改造复杂页面
*   **`SettingsMicrophoneTab` (麦克风)**：包含下拉选框与音频可视化波形，可以利用 `SettingsItem` 的 `action` 区域放入波形图，右侧布局更清晰。
*   **`SettingsSubtitleTab` (字幕设置)** / **`SettingsShortcutsTab` (快捷键)** 等：套用同样的格式，实现全局视觉的一致拉齐。

---

## 探讨与需要您确认的细节 (Open Questions)

1. **抽象粒度**：您是否赞同将现存的 `settings-item`、`settings-label` 完全替代为 `<SettingsItem title="..." hint="..."> {您的控件} </SettingsItem>` 的形式？这会大大削减各 Tab 里的重复代码。
2. **全局 CSS 还是 局部 CSS**：目前的样式分散在个别文件（如 `SettingsLLMServiceTab.css`）。建议新建一个如 `SettingsShared.css` 来定义所有的基础布局，您觉得如何？
3. **首批改造目标**：计划先创建公共组件，再从 `SettingsGeneralTab` 开刀做个示例效果，您是否同意？

如果有其他想要补充的设计方向或者要求（比如希望控件之间留出更大的间距、希望增加某些特定的过渡动画），请随时告诉我！
