# Render Lab

[English](README.md) | 简体中文

<p align="center">
  <img src="media/o3de-attenuation-radius.jpg" alt="O3DE 的 simple point light：它在书房中的辐照度以热力图显示，在衰减半径处降为零，旁边是衰减公式及其数值" width="100%">
</p>

一个用于渲染演示与实验的实验室，按**游戏引擎功能的研究**（investigation）组织。每项研究在预先准备的数据上重建某个引擎功能的一部分，
并逐步展示其中涉及的数据：既在交互式查看器中，也在实验室逐帧录制的旁白视频中。

它使用 Vulkan 上的 **wgpu 30.0.1**、**winit 0.30.13**、**egui 0.37** 和 **WGSL** 着色器。

<p align="center">
  <img src="media/title-screen.jpg" alt="标题画面：书房之上的磨砂玻璃卡片，上有开始、设置和退出" width="49%">
  <img src="media/o3de-point-light.jpg" alt="O3DE 在一点上的 forward pass：一盏 simple point light 照亮的书房，带有物体标注、各项光照和滑块" width="49%">
</p>
<p align="center">
  <img src="media/o3de-tiled-light-culling.jpg" alt="O3DE 的分块光源剔除：每个屏幕 tile 中的光源数量，旁边是剔除公式和所检查 tile 的深度分箱" width="49%">
  <img src="media/o3de-ggx-specular.jpg" alt="O3DE 的 GGX 高光：一个金属球及其 N、L、V、H 向量和高光波瓣，旁边是 D、G、F 公式及其数值" width="49%">
</p>

## 仓库内容

共享的实验室（主页、设置，以及研究所依赖的代码），以及那些许可证允许发布其衍生代码的引擎的研究：

- **Open 3D Engine：光照模型**（[`src/o3de/lighting`](src/o3de/lighting/README.md)）。O3DE 2.8 的 Atom 渲染器如何照亮
  StandardPBR 表面，其着色器代码已移植到 WGSL：metallic 工作流、GGX 高光及其 D、G、F 项、diffuse response、BRDF 纹理与多次散射补偿、
  simple point light，以及全局 skylight 的基于图像的光照（IBL）。

一个本仓库不提供的可选模块可以通过 `commercial` cfg 编译进来（`src/commercial/` 存在时由 `build.rs` 设置）。没有它时，实验室只包含公开的研究。

不带命令启动时，`render-lab` 会打开一个类似游戏的**标题画面**，位于磨砂玻璃之后，有**开始**、**设置**和**退出**三个按钮，左下角显示版权、许可证和版本。

- **开始**打开选择页面：已安装研究中的引擎与功能。
- **设置**编辑 `~/.config/render-lab/config.toml`（或 `$XDG_CONFIG_HOME/render-lab/config.toml`）中的设置；目前只有一项：**全屏**。
  **保存并应用**会在十秒倒计时内试用修改后的设置：**应用**保留它们并写入文件，而**恢复**、Esc 或等待倒计时结束都会还原之前的设置。
  点击应用之前不会写入任何内容。没有需要保存的内容时，第一个按钮是**返回**；与默认值不同的选项会被标记，并带有只恢复该项的**恢复**按钮；
  **放弃**丢弃修改，**恢复默认**显示默认值。
- **F11** 把窗口（主页或查看器）的截图保存到 `build/screenshots/`。

## 命令

在本目录下：

```bash
cargo run --release                                   # 主页
cargo run --release -- home --snapshot home.png       # 离屏渲染标题画面（--page select 或 config 渲染其他页面）
cargo run --release -- home --autotest home.json      # 离屏点击测试主页（使用自己的设置文件）
```

实验室还有 `render`（把时间线录制到 FFmpeg）、`selftest`、`layout` 和 `viewer-autotest`，每个命令都接受
`--investigation ENGINE/FEATURE`（默认：列表中的第一项）：

```bash
cargo run --release -- --investigation o3de/lighting selftest --report o3de.json   # 移植代码对照其 CPU 镜像、画面与源码引用
```

研究的旁白视频由 [`tools/video`](tools/video/README.md) 根据其分镜脚本构建：

```bash
bash tools/video/setup.sh --gpu                                     # 只需一次：在 .cache/ 下安装 Piper、FFmpeg、Node 和 Mermaid
python3 tools/video/build.py --investigation o3de/lighting          # build/video/o3de/lighting/en/o3de-lighting.mp4
python3 tools/video/build.py --investigation o3de/lighting --lang zh-CN
```

屏幕上的文字可以以标签的形式包含公式：在文本表、字幕或读数面板（`Readout::formula`）中写 `{@ formula="\\frac{1}{1+n}"}`。
实验室在运行时自行排版这些公式（`src/lab/formula.rs`：MiTeX 把 LaTeX 转换为 Typst 数学公式，再由 Typst 排版），因此不需要 Node，
也不需要提交图片；`render-lab formulas --check FILE…` 按实验室的限制检查文件中的公式。分镜脚本使用同样的标签，镜头提示按旁白的词语计时
（见 [`tools/video`](tools/video/README.md#text-driven-shots-tags-in-a-sentence)），时间线关键帧的 `formula` 会在视口中打开公式卡片。

## 整体结构

| 路径 | 作用 |
|---|---|
| [`src/main.rs`](src/main.rs) | 命令行。 |
| [`src/lab/home.rs`](src/lab/home.rs)、[`src/lab/frost.rs`](src/lab/frost.rs)、[`shaders/wgsl/frost.wgsl`](shaders/wgsl/frost.wgsl) | 主页：标题、设置和选择页面，背景是标题研究的场景（`investigations::title`，目前为 O3DE 的光照；加载完成前为纯色背景），经过两次减半、模糊、着色并加上颗粒。 |
| [`src/lab/config.rs`](src/lab/config.rs) | 设置文件：它的位置、读取（使用 `toml_edit`）和写入。 |
| [`src/lab/text.rs`](src/lab/text.rs)、[`src/lab/readback.rs`](src/lab/readback.rs)、[`src/lab/gpu.rs`](src/lab/gpu.rs) | 字体与文字、回读画面并保存为 PNG，以及 GPU 设备。文本表、伪代码清单和源码索引都是各层包中的资源，首次使用时读取（`i18n::Table`）。 |
| [`src/lab/formula.rs`](src/lab/formula.rs)、[`src/lab/rich_text.rs`](src/lab/rich_text.rs)、[`assets/mitex/`](assets/mitex) | 公式：LaTeX 由 Typst（通过 MiTeX 的 scope）排版为绘制时着色的覆盖遮罩，以及把带公式标签的文字排在同一条基线上。 |
| [`src/lab/scene.rs`](src/lab/scene.rs)、[`src/lab/scenes/`](src/lab/scenes/study-room.toml)、[`src/lab/terrain.rs`](src/lab/terrain.rs) | 实验室的场景，按 `SceneId` 区分：书房，以及天空下的丘陵地形。每个场景是一个数据文件，即资源 `scene/<key>`（其物体、形状、材质、光源、光照常数、相机从何处观看，以及地形的高度场），在首次构建场景时读取。每个演示显示其中一个（`Investigation::demo_scene`，默认为书房）；渲染器在某帧首次需要时准备场景，并保留它以便切换回来；当一项研究的演示使用多个场景时，查看器为每个场景显示一个标签页，旁边是该场景的演示。地形是网格上的高度场（`SHAPE_TERRAIN`），绘制为三角形，并在 GPU（`intersectTerrain`）和 CPU（`Terrain::intersect`）上逐格追踪；其高度来自给定的采样，或者（可选）固定种子的分形 Perlin 噪声。 |
| `src/lab/*.rs`（其余文件） | 场景渲染器、确定性录制器、查看器及其停靠面板、时间线播放器及其动画（`animation.rs`：缓动、路径、环绕、波动和 Play 片段）、结构体面板的记录（`layout.rs`、`shaders/wgsl/types.wgsl`），以及 `Investigation` trait（`investigation.rs`）。在 `commercial` cfg 下，实验室只把通用扩展点（`Extension`，位于 `investigation.rs`）交给可选模块，别无其他。 |
| [`src/investigations.rs`](src/investigations.rs)、[`src/o3de/`](src/o3de) | 研究列表，以及 Open 3D Engine 的研究。 |
| [`tools/video/`](tools/video/README.md) | 旁白视频构建器：旁白、时间线、渲染、封装、字幕、文档和验证。 |
| [`build.rs`](build.rs) | 检查清单声明的每个 WGSL 模块（每个 `[[shader]]`，包括各研究自己的场景着色）都能通过 naga 验证；根据可选 `commercial` 模块自带的清单决定是否编译它；检查文本表及其公式标签。它不生成任何文件。 |
| [`src/lab/pipeline/`](src/lab/pipeline/mod.rs)、[`src/lab/resources.rs`](src/lab/resources.rs) | 资源管线：实验室挂载各个包（实验室的 `resources.toml`，然后是每个研究的），在运行时按 id 加载资源，按类型处理（`file`；`radiance`：把 HDRI 制作成环境光的各级；`mesh`：把 OBJ 制作成 BVH），并按源文件、参数和处理器的哈希缓存在 `build/resources` 中。`render-lab resources` 列出它们；`render-lab cook` 处理全部资源。 |
| [`src/lab/shaders.rs`](src/lab/shaders.rs)、[`src/lab/shaders.toml`](src/lab/shaders.toml) | 运行时的 WGSL：包声明的每个 `[[shader]]` 由其 `parts` 依次组成（WGSL 资源或其他 shader；实验室的各 pass 模块以 `shader/prelude` 开头），在实验室创建它时组合并由 naga 验证，因此修改后的着色器在下次启动时生效，错误会指出文件和行号。naga 还为结构体面板计算 `wgsl/types` 的布局。可选模块可以在自己的包中覆盖 prelude 的 `wgsl/surface-lookup`。 |
| [`src/lab/demo.rs`](src/lab/demo.rs) | 以数据描述的演示：研究的包声明其参数（`params.toml`）、演示的顺序，以及每个演示一个文件（标题、标签、提示、场景、清单、相机，以及它改变的参数，可共用预设），实验室据此构建每个演示的初始状态。演示绘制什么、其停靠面板和 Play 动画仍是研究的代码（示例见 `src/o3de/lighting/demos/`）。 |
| [`src/lab/loading.rs`](src/lab/loading.rs) | 加载步骤：启动时不读取场景所需的任何内容。查看器打开时（或主页显示背景时），工作线程准备场景的资源、静态数据和渲染器，同时窗口显示进度；`home --snapshot --page loading` 在屏幕外绘制该页面。 |

### 添加研究

一项研究是 `src/<engine>/<feature>/` 中实现 `Investigation`（`src/lab/investigation.rs`）的模块，并在 `src/investigations.rs` 中列出。
凡是从引擎源码衍生的内容都放在研究模块里，不要放进共享的实验室；除非引擎的许可证允许，也不要放进仓库的任何公开副本
（允许时在 [`THIRD_PARTY.md`](THIRD_PARTY.md) 中注明出处）。

以自己的方式为场景着色的研究，从 `Investigation::scene_shading` 返回一个 `SceneShading`：拼接在场景模块之后的 WGSL，
包含一个片元入口、每帧 uniform（`FrameContent::shading`）和每帧数据（`FrameContent::shading_frame`）、静态数据，以及供其 selftest
使用的计算内核（`Renderer::run_shading_verify`）。对于通过多个渲染目标渲染的引擎，它也可以是一条 **pass 链**：片元着色器写入着色的
`outputs`（多重采样目标），其 `passes` 作为计算 pass 在固定绑定的具名目标上运行（`ShadingPass`），最后一个 pass 写入 `"hdr"`，
即画面所显示的内容。在研究的包中把它的 WGSL 声明为一个 `[[shader]]`，其 parts 以 `shader/scene` 开头，并在 `SceneShading::module` 中写上该 shader；build.rs 会验证它，实验室加载时也会验证。没有结构体面板记录的画面可以在原处显示一个
`Readout`（名称与数值）。

## 构建与本地缓存

- **要求：** Rust 1.88 或更新版本，以及 Vulkan 驱动。着色器是 WGSL，因此不需要着色器 SDK。
- **crate 来源：** 新克隆的仓库用普通的 `cargo build` 即可构建，它会从 crates.io 下载锁定版本的 crate。若要把所有 crate 保存在项目内，
  运行一次 [`setup.sh`](setup.sh)（需要联网）：它把 crate vendor 到 `vendor/` 并写入 `.cache/cargo/config.toml`，这样使用
  `CARGO_HOME=$PWD/.cache/cargo` 的构建（视频工具即如此）会离线读取 `vendor/`。[`.cargo/config.toml`](.cargo/config.toml)
  把构建输出放在 `target/`。
- **字体：** DejaVu Sans 和 DejaVu Sans Mono，以及用于中文的思源黑体（Source Han Sans CN），连同它们的许可证一起放在
  [`assets/fonts`](assets/fonts)。
- **环境光：** 照亮书房并显示在其后的天空是 Poly Haven 的 "Tree Lined Driveway" HDRI（CC0），位于
  [`assets/environments`](assets/environments)；资源管线裁剪其中的太阳并缩放亮度（[`src/lab/resources.toml`](src/lab/resources.toml)），
  各研究自行设定其强度。
- **资源：** 实验室加载的文件（环境光、茶壶、字体、MiTeX 的 scope）声明在 [`src/lab/resources.toml`](src/lab/resources.toml) 中，
  在运行时首次需要时处理（格式见 `src/lab/pipeline/mod.rs`）。研究在其代码旁的 `resources.toml` 中声明自己的资源，用
  `Investigation::package` 指明它，并通过 `crate::lab::resources` 读取。
- **声音：** 主页显示时播放循环音乐，点击控件时播放点击音效（演示和视频中都没有），通过 FFplay 播放（视频环境中的 FFplay、`FFPLAY`
  或 PATH 中的）；没有 FFplay 时实验室保持静音。[`assets/audio`](assets/audio) 中的声音由 soundza 授权，不适用 MIT 许可证：
  它们仅供本实验室使用（见 [`THIRD_PARTY.md`](THIRD_PARTY.md)）。

## 许可证

Render Lab 以 [MIT 许可证](LICENSE) 发布，版权所有 © 2026 OppositeNor。随附的字体、茶壶模型、环境光、声音以及 vendor 的 crate
保留各自的条款；见 [`THIRD_PARTY.md`](THIRD_PARTY.md)。
