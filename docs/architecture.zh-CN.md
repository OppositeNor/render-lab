# 架构与代码导览

[Render Lab](../README.zh-CN.md) · [构建](../BUILD.zh-CN.md) · [English](architecture.md)

## 应用与界面

| 路径 | 作用 |
|---|---|
| [`src/main.rs`](../src/main.rs)、[`lab/`](../lab/Cargo.toml)、[`lab/src/cli.rs`](../lab/src/cli.rs) | 二进制程序，只负责运行实验室：实验室是一个 Rust dylib，二进制程序和每个包的库都链接它，因此共享同一个实验室（它的代码和状态）。命令行属于实验室。 |
| [`lab/src/home/`](../lab/src/home/)、[`lab/src/frost.rs`](../lab/src/frost.rs)、[`packages/render_lab/shaders/frost.wgsl`](../packages/render_lab/shaders/frost.wgsl) | 主页：标题、设置和选择页面，背景是标题研究的场景（`investigations::title`，目前为 O3DE 的光照；加载完成前显示加载页面，`home --snapshot --page starting`），经过两次减半、模糊、着色并加上颗粒。 |
| [`lab/src/config.rs`](../lab/src/config.rs)、[`lab/src/volume.rs`](../lab/src/volume.rs) | 设置文件：它的位置、读取（使用 `toml_edit`）和写入；声音的音量（音量值和静音开关）及其滑块，用于设置页面和查看器的面板。 |
| [`lab/src/theme.rs`](../lab/src/theme.rs)、[`lab/src/palette.rs`](../lab/src/palette.rs)、[`themes/`](../themes) | 主题：查找并读取主题文件（构建会把 `themes/` 复制到程序旁边），以及正在使用的界面颜色（`palette::theme()`、`palette::text()` 等），设置主题之前是实验室自己的配色；叠加图形的颜色保持不变。 |
| [`lab/src/loading.rs`](../lab/src/loading.rs) | 加载步骤：启动时不读取场景所需的任何内容。查看器打开时（或主页显示背景时），工作线程准备场景的资源、静态数据和渲染器，同时窗口显示进度；`home --snapshot --page loading` 在屏幕外绘制该页面。 |

## 场景与渲染

| 路径 | 作用 |
|---|---|
| [`lab/src/text.rs`](../lab/src/text.rs)、[`lab/src/readback.rs`](../lab/src/readback.rs)、[`lab/src/gpu.rs`](../lab/src/gpu.rs) | 字体与文字、回读画面并保存为 PNG，以及 GPU 设备。文本表、伪代码清单和源码索引都是各层包中的资源，首次使用时读取（`i18n::Table`）。 |
| [`lab/src/formula.rs`](../lab/src/formula.rs)、[`lab/src/rich_text.rs`](../lab/src/rich_text.rs)、[`packages/render_lab/assets/mitex/`](../packages/render_lab/assets/mitex) | 公式：LaTeX 由 Typst（通过 MiTeX 的 scope）排版为绘制时着色的覆盖遮罩，以及把带公式标签的文字排在同一条基线上。 |
| [`lab/src/scene.rs`](../lab/src/scene.rs)、[`packages/render_lab/scenes/`](../packages/render_lab/scenes/study-room.lam)、[`lab/src/terrain.rs`](../lab/src/terrain.rs) | 实验室的场景，按 `SceneId` 区分：书房，以及天空下的丘陵地形。每个场景是一个数据文件，即资源 `scene/<key>`（其物体、形状、材质、光源、光照常数、相机从何处观看，以及地形的高度场），在首次构建场景时读取。每个演示显示其中一个（`Investigation::demo_scene`，默认为书房）；渲染器在某帧首次需要时准备场景，并保留它以便切换回来；当一项研究的演示使用多个场景时，查看器为每个场景显示一个标签页，旁边是该场景的演示。地形是网格上的高度场（`SHAPE_TERRAIN`），绘制为三角形，并在 GPU（`intersectTerrain`）和 CPU（`Terrain::intersect`）上逐格追踪；其高度来自给定的采样，或者（可选）固定种子的分形 Perlin 噪声。 |
| `lab/src/*.rs`（其余文件） | 场景渲染器、确定性录制器、查看器及其停靠面板、时间线播放器及其动画（`animation.rs`：缓动、路径、环绕、波动和 Play 片段）、故事在包代码中运行的动作（`actions.rs`，用 `#[lab::actions]` 声明）、包的着色器定义、由故事创建实例的 GPU 图像种类（`views.rs`；种类可用 `reads=` 读取包的 pass chain target）、结构体面板的记录（`layout.rs`、`packages/render_lab/shaders/types.wgsl`），以及 `Investigation` trait（`investigation.rs`）。 |

## 包与运行时资源

| 路径 | 作用 |
|---|---|
| [`packages/render_lab/src/`](../packages/render_lab/src/mod.rs) | 标准包的代码，编入实验室，即 `lab::utility`：各项研究共用、不必各自编写的部分，包括画中图及故事按键显示的图像（`inset`）、自检的报告、单帧与测试输入（`selftest`）、包的源码索引及其引用行与源码检出的核对（`sources`）、文本表（`text_table!`），以及演示“播放”动画的采样（`demo`）。 |
| [`lab/src/investigations.rs`](../lab/src/investigations.rs)、[`lab/src/plugin.rs`](../lab/src/plugin.rs) | 研究来自各个包的库。带代码的包写明它的 `library`（一个 cdylib）和 `negotiate` 符号；加载器在运行时打开这个库，用一个 C 结构体（加载器的接口版本、它的实验室构建，以及一个映射表）调用该函数。这个函数由 `lab::negotiate!` 生成，它把这些信息与自己编译时所用的实验室比对，再按名称把包的方法句柄（`investigations`）登记到映射表中，就像 Vulkan 加载器与层的协商。针对另一个实验室构建的库会被拒绝，不会被调用。 |
| [`lab/src/packages.rs`](../lab/src/packages.rs)、[`packages/`](../packages) | 包：实验室的内容以 `packages/` 中的文件夹提供，每个文件夹有一个 `manifest.toml`（名称、作者、联系方式、许可证、它 `import` 的包、它 `export` 的键、带代码时它的 `library` 和 `negotiate` 符号，以及它 `provides` 的内容：资源集合，和演示——一条旁白时间线及其音频）。实验室在启动时扫描它们，并按导入关系排序。包用键指代自己的资源，用 `<package>.<key>` 指代其他包的资源，且只能是它导入的包所导出的键。[`render_lab`](../packages/render_lab/manifest.toml) 包含所有研究共用的内容（天空、茶壶、场景、字体、实验室的 WGSL 和文本）；每个研究有自己的包（`Investigation::package`；[`o3de_lighting`](../packages/o3de_lighting/manifest.toml)）。包是自包含的：它的清单、内容、代码（`src/`、`Cargo.toml`）以及它的工具写出的文件（`build/`：时间线、旁白）都在它的文件夹中，也可以在 `lib/` 中附带已构建的库。`render-lab packages` 列出它们，以及它们的库和句柄。 |
| [`lab/src/pipeline/`](../lab/src/pipeline/mod.rs)、[`lab/src/resources.rs`](../lab/src/resources.rs) | 已构建的包：加载器只读取二进制程序旁边的 `packages/`（`target/release/packages/<name>/`：每个包含清单、资源和库），按完整键（`render_lab.teapot`）读取资源。从源码检出运行时，实验室会先把检出中的包增量构建到那里（[`lab/src/cook.rs`](../lab/src/cook.rs)）：每个资源按类型生成（`file` 和 `directory`：链接；`radiance`：把 HDRI 制作成环境光的各级；`mesh`：把 OBJ 制作成 BVH），只有当它的源文件、参数或处理器改变时才重新生成，因此修改在下次启动时生效。不是从该检出构建的已构建包按原样读取。`render-lab cook` 运行构建并报告结果；`render-lab resources` 列出资源。 |
| [`lab/src/shaders.rs`](../lab/src/shaders.rs)、[`packages/render_lab/shaders.toml`](../packages/render_lab/shaders.toml) | 运行时的 WGSL：包声明的每个 `[[shader]]` 由其 `parts` 依次组成（WGSL 资源或其他 shader；实验室的各 pass 模块以 `render_lab.shader/prelude` 开头），在实验室创建它时组合并由 naga 验证，因此修改后的着色器在下次启动时生效，错误会指出文件和行号。naga 还为结构体面板计算 `wgsl/types` 的布局。 |

## 数值与演示

| 路径 | 作用 |
|---|---|
| [`lab/src/values.rs`](../lab/src/values.rs)、[`lab/src/shader_data.rs`](../lab/src/shader_data.rs)、[`lab/derive/`](../lab/derive/src/lib.rs) | 按键名访问的值：停靠面板、时间轴和故事文件设置的值，每个都带有类型、默认值、范围、单位、标签、名称、颜色和文字。Rust 结构体用 `#[derive(lab::Values)]` 声明它们（相机的 `camera.yaw`、`camera.x` 等；研究的见 `packages/o3de_lighting/src/o3de/lighting/values.rs`）；着色器在 `///` 注释中用 `{@ key=… default=… range=…}` 标签声明其 uniform 结构体的成员，实验室用 naga 读取它们，并每帧打包进缓冲区（标为 `derived` 的成员由研究填写）。这些声明是模式；镜头按键名以各自的类型存储每个值（`ShotState::values`：`state["key"]`、`p`、`set`、`set_value`，向量的分量用各自的键名），并用 serde 与类型化结构体互相转换（`view`、`store`）。时间轴按数值混合它们，开关和索引按其类型取整；时间轴可以把值写成数字、`true`/`false`、向量（`lightPosition=0.4,1.9,0.8`）或选项名，实验室加载时按声明解析（故事的提示通过其词汇写同样的值：`{lightPosition {' 0.4 1.9 0.8}}`、`viewModeNdotl`）。选项值是一个 `#[derive(lab::Choice)]` 枚举，其变体就是选项（`viewMode=ndotl`）：着色器的标签指明它（`choice=ViewMode`），停靠面板列出它的选项，实验室每次加载研究时都会把它的 WGSL 常量写入包中被 git 忽略的 `build/generated/`（着色器组合的一个 `built = true` 资源；构建时的着色器检查在它存在后才检查它）。`render-lab choices` 列出它们。派生的结构体也可以用短键名读写字段（`camera.set_field("yaw", 30.0)`）。`Dock::values`、`Dock::value` 和 `Dock::choice` 根据声明绘制控件；`render-lab values [--label L]` 列出它们。 |
| [`lab/src/demo.rs`](../lab/src/demo.rs) | 以数据描述的演示：研究的包声明其参数（上述的值结构体，或 `params.toml`）、演示的顺序，以及每个演示一个文件（标题、标签、提示、场景、清单、相机，以及它改变的参数，可共用预设），实验室据此构建每个演示的初始状态。演示绘制什么、其停靠面板和 Play 动画仍是研究的代码（示例见 `packages/o3de_lighting/demos/`）。 |

## 工具与构建检查

| 路径 | 作用 |
|---|---|
| [`tools/video/`](../tools/video/README.md) | 旁白视频构建器：旁白、时间线、渲染、封装、字幕、文档和验证。 |
| [`lab/build.rs`](../lab/build.rs)、[`lab/check/`](../lab/check/src/lib.rs) | 每个包的构建检查它自己的着色器（用 naga）和文本表（实验室的构建检查 render_lab）；实验室的构建还为协商计算实验室构建的指纹。它们不生成任何文件。 |

添加新的包见[添加研究](adding-investigations.zh-CN.md)。
