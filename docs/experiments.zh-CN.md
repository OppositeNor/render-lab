# 实验与测量

[Render Lab](../README.zh-CN.md) · [构建](../BUILD.zh-CN.md) · [English](experiments.md)

在仓库根目录运行下列命令。

选择页上的每个功能还有 **实验这个算法**。它打开查看器的 **实验** 标签页，探索演示中也有这个标签页。标签页包括：

- 这项研究的**所有数值**，无论打开的是哪个演示：
  - 引擎自身的设置，每项都标明它对应的属性（`render-lab values` 中标为 `engine …`）；
  - 场景的数值；
  - 演示的呈现。
- **与默认值的差异**：被标出，可逐项重置。在实验模式下，这些改动会带到下一个演示。
- 与保存的参考做 **A/B 对比**：划分、并排、差异或仅参考，每个视图保留各自的历史。
- 差异的**测量**：PSNR、SSIM、ꟻLIP、MSE 和相对 MSE。
- **每个渲染阶段的 GPU 耗时**，以及右侧的**性能面板**（每帧 GPU 耗时、其历史与各阶段条形图），旁白故事也可以显示它（`panel=performance`，`highlight=<阶段>`）。
- **帧图**（停靠栏的帧图标签页；`panel=dag`、`panel=graph`）：按运行顺序展示每个 pass、它们读写的资源、资源生命周期及对应的引擎 pass。
  点击某个 pass 的资源，可检查该 pass 刚结束时的内容（故事中用 `imageGraph=<pass>.<resource>`，实验中用 `[inspect]`）。
  **DAG** 视图把每个 pass 显示成一张带结果图的卡片，用箭头连接资源；可拖动画布或卡片调整位置。**资源**视图把帧结束时的每个资源显示成卡片。
  全屏时，pass 的**显示着色器源码**会在图旁显示实际编译的 WGSL；查看器支持实时重载，编辑 WGSL 文件会重新编译管线。
- 可保存和载入的**实验文件**。

## 离屏研究实验

同一个文件也可以离屏作为研究实验运行：

- **运行：** 参考、参数扫描与消融。
- **计时：** 先预热，再重复计时，每帧按渲染阶段测量 GPU 耗时。
- **输出：** 一个目录，包含 CSV 与 JSON 结果、PNG 与 PFM 图像、ꟻLIP 误差图，以及记录运行内容的清单（实验、提交、GPU 与驱动）。

[`tools/experiments`](../tools/experiments/README.md) 把该目录整理成表格、图表和图像网格：

```bash
cargo run --release -- --investigation godot/voxelgi experiment godot_voxelgi.experiment/cones-and-bias --out runs/vgi
python3 tools/experiments/report.py runs/vgi        # runs/vgi/report/report.html
```

文件格式见 `lab/src/experiment.rs`，实验方法（参考、对照、重复）见 `experiments` 技能。
