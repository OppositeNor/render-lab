# 添加研究

[Render Lab](../README.zh-CN.md) · [构建](../BUILD.zh-CN.md) · [English](adding-investigations.md)

一项研究是 `packages/<engine>_<feature>/` 中的一个包：它的内容，以及它的代码——一个实现 `Investigation`（`lab/src/investigation.rs`）、并在协商时把它交给实验室的库（参照 [`packages/o3de_lighting/src/lib.rs`](../packages/o3de_lighting/src/lib.rs)，并把这个包加入 `Cargo.toml` 的 workspace）。
凡是从引擎源码衍生的内容都放在研究的包里，不要放进共享的实验室；除非引擎的许可证允许，也不要放进仓库的任何公开副本
（允许时在 [`THIRD_PARTY.md`](../THIRD_PARTY.md) 中注明出处）。

以自己的方式为场景着色的研究，在其包的 `shading.toml` 中声明它（一个 `shading` 资源，运行时读入为 `SceneShading`；格式见
[`lab/src/shading_file.rs`](../lab/src/shading_file.rs)），并用 `Investigation::shading_hooks` 注册文件中按名字引用的函数，即那些计算而非声明的部分（静态数据、更新或程序的计划）。着色是拼接在场景模块之后的 WGSL，
包含一个片元入口、每帧 uniform（`FrameContent::shading`）和每帧数据（`FrameContent::shading_frame`）、静态数据，以及供其 selftest
使用的计算内核（`Renderer::run_shading_verify`）。对于通过多个渲染目标渲染的引擎，它也可以是一条 **pass 链**：片元着色器写入着色的
`outputs`（多重采样目标），其 `passes` 作为计算 pass 在固定绑定的具名目标上运行（其 `[[node]]` 列表中的 `compute` 节点，按帧的顺序），最后一个 pass 写入 `"hdr"`，
即画面所显示的内容。在研究的包中把它的 WGSL 声明为一个 `[[shader]]`，其 parts 以 `render_lab.shader/scene` 开头，并在文件的 `module` 中写上该 shader；build.rs 会验证它，实验室加载时也会验证。没有结构体面板记录的画面可以在原处显示一个
`Readout`（名称与数值）。

相关代码的位置见[架构与代码导览](architecture.zh-CN.md)。
