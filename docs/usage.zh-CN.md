# 使用 Render Lab

[Render Lab](../README.zh-CN.md) · [构建](../BUILD.zh-CN.md) · [English](usage.md)

## 主页与设置

不带命令启动时，`render-lab` 先在标题画面背后的场景加载时显示加载页面，然后打开一个类似游戏的**标题画面**，位于磨砂玻璃之后，有**开始**、**设置**和**退出**三个按钮，
右上角是地球图标和当前语言（点击后打开设置页面的语言选项），左下角显示版权、许可证和版本。启动时先以设置的语言显示光敏性癫痫警告，点击**继续**后关闭；
保存的设置改变语言时，会再以新语言显示一次。

- **开始**打开选择页面：已安装研究中的引擎与功能。每个功能都可以观看讲解视频、探索演示、做实验；它的包带有测验时，还可以**参加测验**：关于它的故事的选择题，每道题显示在它所问的演示的实时画面右边，可以转动、点击并调整这个画面来找到答案，最后给出得分。
- **设置**编辑 `~/.config/render-lab/config.toml`（或 `$XDG_CONFIG_HOME/render-lab/config.toml`）中的设置，每个类别一个标签页：
  - **界面**：**语言**（English 或中文：主页的文字，以及选择页面首先提供的旁白视频版本，某个特性没有这种语言的版本时使用英文版；演示也使用该语言）
    和**主题**，即主页和查看器的配色（见下）。
  - **渲染**：**全屏**。
  - **声音**：**音乐**和**旁白**的音量，每个音量都是一个滑块加一个静音按钮（静音时保留音量）。查看器的面板，以及视频播放时的时间轴，也可以设置旁白的音量；
    查看器关闭时会把它写入文件。

  **保存并应用**会立即应用界面和声音设置并写入文件。渲染设置的更改则先在十秒倒计时内试用：**应用**保留它并写入文件，而**恢复**、Esc
  或等待倒计时结束都会还原之前的设置。点击保存并应用之前不会写入任何内容。没有需要保存的内容时，第一个按钮是**返回**；有修改的标签页带一个圆点，
  与默认值不同的选项会被标记，并带有只恢复该项的**恢复**按钮；**放弃**丢弃修改，**恢复默认**显示默认值。
- **主题**是 TOML 文件，每个主题一个，以文件名（不含扩展名）命名（设置中写作 `theme = "tokyo-night"`）。实验室在运行时从程序旁边的
  `themes/` 读取它们，构建会把自带的主题（来自 [`themes/`](../themes)：**One Dark Pro**、**Tokyo Night** 和 **VS Code**）放在那里；
  也从 `~/.config/render-lab/themes/` 读取，其中的文件取代程序旁边的同名文件。主题给出自己的名称并设置界面的每一种颜色；
  [`themes/one-dark-pro.toml`](../themes/one-dark-pro.toml) 说明了每个键。实验室自己的配色是内置主题 **Render Lab**，它是默认主题，录制始终使用它，
  因此录制的画面从不依赖设置。
- **每个滑块、复选框和选项**的名称后都有一个 **?**：把指针移到上面，会用界面语言说明这个数值的作用。
- **查看器的布局**可以用鼠标拖动来调整：拖动视口与面板之间的边缘（结构面板和代码标签页的列表各自记住宽度），以及停靠栏上方的边缘。
  每次拖动结束时，尺寸会写入设置文件，下次打开查看器时会恢复。双击某条边缘可恢复该尺寸，停靠栏的 **重置布局** 恢复全部尺寸，
  设置页的 **恢复默认** 也会恢复。录制始终使用默认布局。
- **F11** 把窗口（主页或查看器）的截图保存到 `build/screenshots/`。

## 命令

在仓库根目录运行：

```bash
cargo run --release                                   # 主页
cargo run --release -- --list                         # 已安装的研究（--investigation 可用的取值）
cargo run --release -- home --snapshot home.png       # 离屏渲染标题画面（--page select、config、starting、viewer、viewer-timeline、viewer-experiments、viewer-compare 或 viewer-quiz 渲染其他页面）
cargo run --release -- home --autotest home.json      # 离屏点击测试主页（使用自己的设置文件）
```

实验室还有 `render`（把时间线录制到 FFmpeg）、`selftest`、`layout` 和 `viewer-autotest`，每个命令都接受
`--investigation ENGINE/FEATURE`（默认：列表中的第一项）：

```bash
cargo run --release -- --investigation o3de/lighting selftest --report o3de.json   # 移植代码对照其 CPU 镜像、画面与源码引用
```

## 旁白视频

研究的旁白视频由 [`tools/video`](../tools/video/README.md) 根据其分镜脚本构建：

```bash
bash tools/video/setup.sh --gpu                                     # 只需一次：在 .cache/ 下安装 Piper 和 FFmpeg
python3 tools/video/build.py --investigation o3de/lighting          # packages/o3de_lighting/build/video/en/o3de-lighting.mp4
python3 tools/video/build.py --investigation o3de/lighting --lang zh-CN
```
