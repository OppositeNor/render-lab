# 构建 Render Lab

[Render Lab](README.zh-CN.md) · [English](BUILD.md)

路径和命令均相对于仓库根目录。

## 要求

Rust 1.100 或更新版本，以及支持 Vulkan 的 GPU 驱动（Windows 上也可用 Direct3D 12，macOS 上用 Metal）。由 wgpu 选择图形 API；`WGPU_BACKEND`（`vulkan`、`dx12`、`metal`）可指定其一。着色器是 WGSL，因此不需要着色器 SDK。

## 从源码构建

普通构建会获取锁定的依赖，并构建二进制程序、共享实验室及公开包的库：

```bash
cargo build --release --locked
cargo run --release
```

## 离线构建与本地缓存

联网运行一次 [`setup.sh`](setup.sh)，把锁定的依赖保存到 `vendor/`，并写入 `.cache/cargo/config.toml`。
之后，使用 `CARGO_HOME=$PWD/.cache/cargo` 的构建（包括视频工具）会离线读取 `vendor/`。
[`.cargo/config.toml`](.cargo/config.toml) 把构建输出放在 `target/`。

```bash
bash setup.sh
export CARGO_HOME="$PWD/.cache/cargo"
cargo build --release --locked --offline
```

如果可选模块加入了有独立工作区的本地包，先构建上面的共享实验室，再替换下例中的 `NAME` 构建各包：

```bash
cargo build --release --locked --offline --manifest-path packages/NAME/Cargo.toml
```

## 验证

配置好本地缓存后，运行单元测试与离屏检查：

```bash
export CARGO_HOME="$PWD/.cache/cargo"
cargo test --release --locked --offline --workspace
cargo run --release --locked --offline -- home --autotest /tmp/render-lab-home.json
cargo run --release --locked --offline -- --investigation o3de/lighting selftest --report /tmp/render-lab-o3de.json
```

离屏检查需要这样的驱动。更多命令与快照入口见[使用 Render Lab](docs/usage.zh-CN.md#命令)。
视频的环境与检查见[视频构建器](tools/video/README.md)，报告工具见[实验报告](tools/experiments/README.md)。

## 运行时资源与声音

- **字体：** DejaVu Sans 和 DejaVu Sans Mono，以及用于中文的思源黑体（Source Han Sans CN），连同它们的许可证一起放在
  [`packages/render_lab/assets/fonts`](packages/render_lab/assets/fonts)。
- **环境光：** 照亮书房并显示在其后的天空是 Poly Haven 的 "Tree Lined Driveway" HDRI（CC0），位于
  [`packages/render_lab/assets/environments`](packages/render_lab/assets/environments)；资源管线裁剪其中的太阳并缩放亮度（[`packages/render_lab/resources.toml`](packages/render_lab/resources.toml)），
  各研究自行设定其强度。
- **资源：** 实验室加载的文件（环境光、茶壶、字体、MiTeX 的 scope）由 [`render_lab`](packages/render_lab/manifest.toml) 包声明，
  在运行时首次需要时处理（格式见 `lab/src/packages.rs` 和 `lab/src/pipeline/mod.rs`）。研究在自己的包中声明自己的资源，用
  `Investigation::package` 指明它，并通过 `lab::resources` 读取。
- **声音：** 主页显示时播放循环音乐，点击控件时播放点击音效（演示和视频中都没有），通过 FFplay 播放（视频环境中的 FFplay、`FFPLAY`
  或 PATH 中的）；没有 FFplay 时实验室保持静音。[`packages/render_lab/assets/audio`](packages/render_lab/assets/audio) 中的声音由 soundza 授权，不适用 MIT 许可证：
  它们仅供本实验室使用（见 [`THIRD_PARTY.md`](THIRD_PARTY.md)）。
