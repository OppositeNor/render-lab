# 前置知识与公式

[Render Lab](../README.zh-CN.md) · [构建](../BUILD.zh-CN.md) · [English](learning-content.md)

路径和命令均相对于仓库根目录。

## 前置知识

这些算法需要大量前置知识，因此实验室维护一张**知识图谱**：每一章需要的知识，一直追溯到三个基础（GPU 并行执行；辐射度量学；
图形渲染管线）。每项研究写明它面向的观众，由此把知识图谱分成观众应当已知的部分、故事的起点，以及故事要讲解的部分。

- 查看器的**前置知识**标签页列出当前章节（或当前演示所属章节）需要预先了解的知识（按领域分组）、故事的起点，以及故事讲解的内容。
- 旁边的**知识图谱**面板画出整张图，按观众已知的程度着色；可拖动、滚轮缩放，点击一个知识点查看说明。**全屏**按钮让它覆盖整个窗口（按 Esc 关闭）。
- 主页上每个功能的**开始之前**列出故事的起点和它直接用到的已知知识点，并有打开知识图谱的按钮。

知识图谱在 `packages/render_lab/knowledge.toml`，每项研究的章节与观众在它自己的 `knowledge.toml`，方法见 `knowledge-graph` 技能：

```bash
cargo run --release -- --investigation godot/sdfgi knowledge         # 检查知识图谱，并按章节列出观众的划分
```

## 公式

屏幕上的文字可以以标签的形式包含公式：在文本表、字幕或读数面板（`Readout::formula`）中写 `{@ formula="\\frac{1}{1+n}"}`。
实验室在运行时自行排版这些公式（`lab/src/formula.rs`：MiTeX 把 LaTeX 转换为 Typst 数学公式，再由 Typst 排版），因此不需要 Node，
也不需要提交图片；`render-lab formulas --check FILE…` 按实验室的限制检查文件中的公式。分镜脚本使用同样的标签，镜头提示按旁白的词语计时
（见 [`tools/video`](../tools/video/README.md#text-driven-shots-cues-in-a-sentence)），时间线关键帧的 `formula` 会在视口中打开公式卡片。
