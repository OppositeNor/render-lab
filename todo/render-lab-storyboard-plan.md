# Render Lab storyboard 修改计划

日期：2026-11-04。性质：Render Lab storyboard 写作链路的设计与任务清单；PR 1–3 已实施，后续阶段待实施。

## 实施进度（2026-11-04）

- PR 1：增加 `tools/video/check.py`，使用已构建 host，在系统 Python 下检查真实故事及翻译，不导入音频或渲染链路。`--json` 输出版本化诊断。
- 宿主增加 `story check FILE`／`story check --timeline FILE`，`-` 读取 stdin；未计时 cue 保留符号 Length，与运行时共用值、引用和 motion 形状检查，不执行 action。
- 构建在选择旁白设备／合成之前运行 preflight；实测时间线在写出准备产物之前经过完整宿主校验。原运行时继续验证直接提供的时间线。零时长沿用瞬时完成语义，负 motion 时长被拒绝。
- cue 的文件、行、列和范围传到诊断；最终时间线旁的 `timeline.provenance.json` 单独保存来源，原 `source` beat ID 与播放数据保持兼容。
- 验证：30 项 Python 测试、101 项 Rust 单元测试通过；正常构建、各本地包构建、公开文件独立副本构建无警告。公开 8 个语言版本的时间线、字幕、旁白、文档和 IR 经现有 compare 工具比较均一致。
- 公开调查各取 12 个 beat 中点，共 48 帧，与父提交构建比较解码像素；差异逐张复渲染，排除任务开始前已有的内容修改后，46 帧完全一致，2 帧仅实测计时读数／性能曲线不同，场景像素一致。
- 位置限制已记录：当前 IR 的 span／range close 没有独立位置，使用明确标注的容器回退；旧 host 的 Lamlet 编译失败只输出文本诊断，结构化字段保留未知值和回退说明。后续可向上游要求独立 span 位置及机器可读编译诊断，不伪造列号。

- PR 2：`headings.py` 在任何标题属性、章节或翻译 beat 字典化之前检查 schema、重复属性、重复／缺失 chapter 与 beat ID、重复章节号及空结构。章节号要求正整数且唯一，不要求连续；重复诊断在 `related` 中保存第一处声明，属性位置明确回退到标题文本所在行。
- 标题字段拼写错误给出候选项；原有 `engineVersion`、`commit`、`date` 元数据及显式 `meta.*` 注解被保留。翻译标题使用自己的字段集，原有句数、章节号及公式对应检查继续工作。cue 内及 cue 间的覆盖顺序不变。
- 数值要求 finite；整数输入按原始十进制检查，不再静默截断。fps、尺寸、captionWidth、章节号及 cue 的 term／record 序号检查类型和值域，尺寸复用 CLI 规则。target、hold 和 pacing 的零值保留既有语义；时间须能换算为 64 位帧数，超大权重不能溢出暂停分配。字幕宽度还按实际文本检查不能容纳的长 token，避免在合成后报错或陷入分割循环。
- PR 2 验证：51 项 Python 测试通过，包括 `python -O` 的结构诊断、两处重复来源、负例在 synthesize 前失败、元数据与 cue 覆盖兼容；公开 8 个版本的时间线、字幕、音频、文档和 IR 经 compare 工具比较均一致。正常构建及各本地包构建无警告；公开调查共 8 张离屏代表画面已查看。未修改 Rust、渲染资源或故事内容。

- PR 3：新增 `highlightPasses`、`highlightCode`、`highlightFields`，保留显式 namespace 到时间线；旧字符串列表兼容。共享宿主检查按实际 demo／panel／listing／record 继承状态验证高亮，切换消费者时重新检查。图面板使用 node 名，性能面板使用 timing 名，代码 tag 只在指定 listing 中查找，字段只在当前 record 中查找。
- 旧高亮在没有消费者时给 `story.highlight.ambiguous` warning，不改变既有播放；显式目标即使隐藏也检查存在。String helper、常量和 raw attr 的最终引用共用检查，错误包含类别、上下文及邻近拼写；补齐 stop、hide、raw source／text key、graph image click、PNG 资源及封闭选项检查。
- 清理可引用全文中稍后声明的同类实例，合法但未激活的名称允许幂等清理；参数更新仍要求实例先创建。arrow／label 共用运行时实例名，切换类型须完整声明新类型，同一 cue 不可重复定义同名 note；清理旧类型不会隐藏已改成另一类型的实例。
- PR 3 验证：104 项 Rust 测试、60 项 Python 测试通过；正常、各本地包及公开文件副本构建无编译警告。公开 8 个版本的时间线、字幕、音频、文档和 IR 与修改前相同；当前管线使用父／新 host 的完整 consumer regression 也通过。公开调查各 12 个 beat 中点共 48 帧已比较解码像素并查看，差异帧单独重渲染后 46 帧完全相同，另 2 帧仅实测耗时读数／性能曲线变化，场景像素相同。另有新入口离屏预览：代码高亮像素完全等同旧写法，pass 入口仅图面板计时数字不同，高亮行相同。
- 字段验证的边界：检查的是 WGSL schema／field 及请求的 `record` 上下文，不保证当前帧显示该 record。实际显示还需调查的 `build_frame` 提供 `panel_record`，当前公开 demo 没有该 producer；字段入口已验证真实 Lamlet 数据、引用规则、运行时名称与兼容输出，但没有可见字段高亮的公开视觉 fixture。预检不执行 `build_frame`；要精确预测显示能力，需后续增加纯 metadata 契约。二维 beat 也可能保留 panel 设置而实际显示 diagram／source card。

下一阶段是 PR 4 的 motion、range、状态和时间契约；PR 4–7 尚未实施（2026-11-05 复核：f6468d9 只清理代码，未涉及 PR 4–7）。文中引用的行号为制定计划时的位置，PR 1–3 之后多数已偏移，实施时以符号重新定位。下文的源码审计与复现记录保留为计划制定时的依据，不代表后续阶段已经完成。

范围是本次反馈涉及的 Lamlet storyboard 写作、引用、校验、计时和预览链路；不是整个渲染器的缺陷清单。下列“已确认”来自当前源码和独立 Python 复现；“编辑负担”是从现有写法可直接观察到的成本；涉及产品取舍的地方单独标为“需决定”。没有把推测写成已发生的故障。

文中的路径均相对于 Render Lab 仓库。当前依赖为 Lamlet v0.4.0、lamlet-present v0.1.4；实施时重新核对 `lab/Cargo.toml` 和 `Cargo.lock`。

## 目标与边界

作者修改一段旁白、一个高亮或一条相机轨迹时，应当能快速表达意图、在语音合成前发现不依赖实测时长的错误，并定位到对应源码。准备阶段必须产出已经通过宿主检查的时间线。局部检查和预览应能直接按 chapter／beat 选择。

工作主要落在 `render_lab` Lamlet 包、调查项目的 story 库、Python storyboard 读取器、Rust 语义检查器和命令行工具。先用现有 Lamlet 函数、记录、variant 和模式宏解决；本计划不要求修改 Lamlet 解析器、HM 推断、D1 或通用 `std`。

领域封装分工：通用演示机制有充分复用证据时才考虑 `present`；Render Lab 的镜头和资源意图放在 `render_lab`；单个视频的基线、常用镜头和复合动作留在其 `video/story.lam`。

## 已有能力，实施时需要保留

- `focus` 已在 `tools/video/storyboard.py:67` 的 `validate` 中按 graph／diagram 检查（`build.py` 导入并调用），发生在语音合成之前。
- `cue "key"` 中的 `key` 是提示类型，不是某个 cue 的唯一 ID。`presentation.py:210` 拒绝错误的 point cue 类型；不需要给每个 `key` 建 ID 注册表。
- source key、callout、text key、point 和 formula 已有不同层次的检查。`lab/src/script/natives.rs:28` 提供部分检查原语。
- 缺失 chapter／beat ID、重复英文 beat ID、部分翻译对应关系和公式对应关系已检查。
- `opening`、`baseRoom`、`lowCamera` 等复用已经存在于 `packages/godot_ssr/video/story.lam`，不是从零建立库。
- `path` 已检查 waypoint 数量与 channel 数量相等；Rust 已检查空轨迹、orbit 形状、wave 正周期等。应前移或复用检查，不重复实现互相分歧的规则。
- 已有 `--binary`、`--cached-audio-only`、`--prepare-dir`、`--frames A:B`、`--snapshots-only`、IR／deck、`compare.py` 和 `regression.py`。新增工具应补齐它们之间的工作流，不重新建设另一套输出和回归框架。

## 痛点清单

P0：可能漏错、晚报错或静默改变结果。P1：高频编辑和诊断负担。P2：后续维护和较大范围的体验改进。

### 写作接口

| ID | 痛点与现状 | 性质／优先级 | 改进方向 |
|---|---|---|---|
| RL01 | 单个按句运动也需要经过 `cue → 属性列表 → motion → motion 列表 → path`，高频操作出现大量重复结构。 | 编辑负担，P1 | 增加表达“设置关键帧”“按句运动”“切镜头”等意图的薄封装；优先函数，确需改写调用形状时用宏。 |
| RL02 | 相机 waypoint 是嵌套 `List(Float)`，yaw／pitch／distance 依赖位置，`{'` 的列表标记不醒目；插行、调序和检查某个坐标困难。 | 编辑负担，P1 | 提供相机 pose 构造器或逐行 waypoint 宏，使一行就是一个点；转换仍产出原轨迹数据。保留通用 path。 |
| RL03 | `cameraPath`／`cameraTurn` 固定使用 `beat`；换成 `sentence` 要退回通用 path 并重复 channel 清单。现有 `lasting` 只支持 wave，不能直接改 path 的长度。 | 已确认接口限制，P1 | 增加带 Length 的相机轨迹形式，保留旧函数的 beat 默认值。单位和作用范围在调用或帮助中可见。 |
| RL04 | `opening` 是 story 局部配方，只设置 demo／panel、清 callout／inset；它不代表清理所有旁白状态。demo 切换又有自己的继承规则，作者需要记住这些组合。 | 编辑负担／需决定，P1 | 明确 baseline、override、carry、reset 的契约；提供显式清理组和镜头配方，并在 explain 中显示继承。不要默认改成“每 beat 全清”。 |
| RL05 | 标题属性用 CommonMark 的 `key=value`，cue 用 Lamlet 函数；再加裸 `attr`，作者面对几种查找、取值和报错方式。 | 编辑负担，P2 | 给三种入口共用的属性说明、类型和示例；保留各自现有语法，不为了表面统一而重写标题格式。 |

依据：`packages/render_lab/language/render_lab.lam:73–164`、`packages/godot_ssr/video/storyboard.lam:7–13`、`packages/godot_ssr/video/story.lam:9–19`、`lab/src/shot.rs:1075–1130`。

### 引用与语义检查

| ID | 痛点与现状 | 性质／优先级 | 改进方向 |
|---|---|---|---|
| RL06 | `highlight` 只检查名字不含逗号，不检查目标存在；拼错 pass 或 listing tag 时，界面可能只是没有东西亮。 | 已确认，P0 | 在语音合成前检查目标；错误包括目标类别、名称、上下文和候选项。 |
| RL07 | 同一个 `highlight` 承载 pass、代码标签和其他面板的字段等含义。相同字符串在不同上下文中可能合法，也可能误指。 | 已确认设计负担，P1 | 增加有命名空间的高亮入口，例如按 pass、listing tag、record field 区分。原接口兼容期按当前状态验证，歧义给诊断。不要只检查所有名字的并集。 |
| RL08 | vocabulary 有合法 option 常量，但相应 String 函数并不验证成员资格。合法的 String 类型不等于合法的 panel／demo／listing 等值。 | 已确认，P0 | 区分封闭选项、资源引用、story 自行声明的实例名；在公共语义层检查结果，不能只依赖常量。 |
| RL09 | 裸 `attr` 可以绕过生成函数。`markup.cue_key` 把未知属性回退成 params，拼错名字可能直到 Rust 读取时间线才报错。 | 已确认，P0 | 用完整 schema 识别参数与扩展属性；保留受控扩展能力，未知名字尽早报错并给建议。 |
| RL10 | action 名称／参数、image kind／graph resource／参数、值的维度等已有 Rust 校验，但发生在时间线加载时。`--prepare-only` 可在调用这些校验前成功退出。 | 已确认，P0 | 抽取可复用的、无 GPU 的语义检查；不依赖时间的部分前移到 TTS 前，完整 Timeline 校验放到准备完成前。 |
| RL11 | 同一 key 内多个 motion 驱动相同 channel 时，当前逐个写 segment，后者覆盖前者；没有 motion 与 motion 的重复 channel 检查。显式 set 与 animate 冲突则已有检查。 | 源码确认覆盖行为，规则需决定，P0 | 默认拒绝意外重复 channel；若要允许叠加或覆盖，要求显式表达。实施前审计现有故事是否使用该行为。 |

依据：`render_lab.lam:193–219`、`lab/src/script/keys.rs:58–163`、`tools/video/markup.py:60–69, 221–268`、`lab/src/shot.rs:983–1048, 1221–1232`、`lab/src/pseudocode.rs:170`、`lab/src/ui.rs:1221`。生成 vocabulary 的 String 函数只做 String ascription，合法 option 额外生成常量；需检查当前锁定版本的生成器，不能把“有 options”当作成员验证。

### 结构与数值

| ID | 痛点与现状 | 性质／优先级 | 改进方向 |
|---|---|---|---|
| RL12 | chapter ID 唯一性未检查；章节编号用 `int(number(...))`，`1.7` 会变成 `1`，正整数／唯一性也未统一约束。 | 已复现，P0 | 检查 ID 唯一、编号为正整数且唯一；报出冲突的两处位置。无需强迫编号必须连续，除非产品明确要求。 |
| RL13 | 翻译中的重复 beat ID 在构建字典时覆盖；重复 chapter ID 在 localize 构建字典时也可能覆盖。现有对应关系检查无法找回被覆盖项。 | 源码确认；beat 覆盖已复现，P0 | 在字典化之前检查重复项；保留已有缺项、多项、句数和公式对应检查。 |
| RL14 | beat 属性有未知名称检查；video／chapter 属性没有一致的检查，拼写错误可被保留或忽略。部分重复标题属性在 dict 化后只保留末值。 | 已确认；header 拼错已复现，P0 | 建立各级标题 schema；区分内建属性和合法元数据扩展。重复标题属性报错。不要误伤已有 engineVersion／commit／date 等元数据。 |
| RL15 | Python 数值读取允许 NaN／Infinity；负 target 可进入 board，fps／pacing 的类型和值域缺少统一检查。浮点转整型也可能掩盖错误。 | 部分已复现，P0 | 明确 finite、整数性、正值／非负规则；复用已有分辨率约束。不要把所有参数都按 UI slider 的建议范围硬截断。 |
| RL16 | 空 chapter 可通过循环式 validate；缺少完整故事／章节非空检查。现有 beat 无 narration 检查应保留。 | 源码确认，P0 | 对空故事、空章节和无有效旁白的 beat 给结构诊断；若支持特殊空章节，需显式声明。 |

依据：`tools/video/presentation.py:254–293, 303–387`、`tools/video/build.py:83–109`、`tools/video/zh.py:30–65`、`tools/video/markup.py:129`、`tools/video/timeline.py:30–47`。分辨率 CLI 已有校验，位于 `tools/video/config.py:32`；本项是统一所有入口，不是声称整个项目没有校验。

### 计时、状态与翻译

| ID | 痛点与现状 | 性质／优先级 | 改进方向 |
|---|---|---|---|
| RL17 | `sentence` 实际按 paragraph 计句段，并从 cue 到下一句段开始／beat 结束，包含排期留白，不等于当前发音结束。改段落或 pacing 会改变运动时长。 | 已确认语义，P1 | 文档和 explain 显示精确边界；保留旧语义。如果确有“到发音结束”的需求，另加明确长度类型，不改旧 sentence。 |
| RL18 | 同一 channel 的嵌套／重叠 range、point 和 motion 会依顺序覆盖，作者难从源码判断最终效果。跨 key 接管通常是正常用法。 | 源码确认顺序行为；诊断策略需决定，P1 | explain 显示 channel 写入和接管；对可疑的同区间冲突给 lint。不要把所有后续接管都判错。 |
| RL19 | motion 的形状和数值检查分散：waypoint 维度在 Lamlet 检查，orbit start／axis、wave period 等在 Rust 检查；负 numeric seconds 可进入 Python 计时结果。yaw／distance／seconds 都是 Float。 | 已确认，P0／P1 | P0 前移形状、finite、负值检查；明确零时长是否允许瞬时切换。P1 在 API、schema 和帮助中注明单位。暂不为每种单位扩充语言类型系统。 |
| RL20 | 章节 target 是预算；音频时长超预算的结果主要到后续验证才暴露。当前从实测时长计算暂停，不会让过长旁白自动满足预算。 | 已确认，P1 | 实测音频可用后立刻报告预算、自然时长、暂停分配、超出量，在渲染前决定警告或失败。不能承诺 TTS 前知道精确时长。 |
| RL21 | 翻译按 beat ID 和句段位置对应；英文拆／并一个 paragraph 就要求翻译相应调整，错误只报数量时不够好找。 | 编辑负担，P1 | 先增强对齐报告，显示两侧句段及位置。稳定 sentence ID 作为后续可选设计，不是首轮前置条件；当前中文版本沿用英文配音和时间线。 |

依据：`tools/video/timeline.py:101–129, 149–196, 268–294`、`lab/src/animation.rs:257–268`、`tools/video/zh.py:41–64`、`tools/video/verify.py:89–96`。

### 诊断、检查与维护

| ID | 痛点与现状 | 性质／优先级 | 改进方向 |
|---|---|---|---|
| RL22 | Python `where()` 把 IR 的位置压成 file:line；tag 不保存独立位置，最终 key 的 `source` 只剩 beat ID。后期错误难定位到某个 cue／列。 | 已复现／源码确认，P0 | 保留文件、行、列和范围；在 parser、语义检查、timing、timeline 间传递。容器若没有精确位置，应明确标注回退位置，不伪造列号。 |
| RL23 | 缺少一个不依赖 TTS 环境的完整 story check。`--prepare-only` 仍加载语音环境、合成或读音频缓存，并生成整套准备产物。 | 已确认，P0 | 增加独立无音频检查入口；支持已有 host binary；不初始化 GPU，不读取配音模型。 |
| RL24 | 已有 IR 和 prepared timeline，但没有按 beat 汇总展开 cue、资源、继承状态、channel 写入和时间边界的直接入口。 | 编辑负担，P1 | 增加 beat explain；无测量时输出符号时长，有缓存时再显示实测时间和帧。 |
| RL25 | 现有快照取每 beat 中点，能漏掉瞬时 cue、错误高亮或运动端点；`--frames` 需要作者先找帧号。 | 已确认工具限制，P1 | 按 beat／cue／局部时间选择预览；补 cue 前后和运动端点采样，复用现有 offscreen renderer 和 frames 功能。 |
| RL26 | vocabulary、typed 属性名单、字符串／CSV／JSON 序列化、Python 解析和 Rust 检查分散维护；生成器会省略部分冲突名称或 option，最终需要 attr 绕行，但发现过程不直观。 | 已确认维护负担，P2 | 提供 schema 和命名冲突报告；集中字段定义、选项、单位和 validator 信息，逐步减少手工名单和字符串往返。第一阶段不重写全部 IR。 |

依据：`tools/video/presentation.py:112–125, 144–217`、`tools/video/timeline.py:132–145, 286–294`、`tools/video/build.py:184–240`、`tools/video/outputs.py:164–173`、`lab/src/script/keys.rs:40–48, 129–163`、`packages/render_lab/language/render_lab.lam:60–71, 193–201`。

## 分阶段实施计划

### PR 1 — 建立可独立运行的检查入口与位置传递（P0）

覆盖 RL10、RL22、RL23，为其他 P0 工作提供共同入口。

1. 从 `build.py` 提取或复用 `load_board` 和 board validate。建议新增 `tools/video/check.py`，不导入／初始化 TTS、phonemizer、FFmpeg 或 GPU。以下命令是建议接口（PR 1 已实现 `check.py` 的 `--investigation`、`--binary`、`--lang`、`--json`；`--explain` 等属 PR 6，尚未实现）：

   ```sh
   python3 tools/video/check.py --investigation godot/ssr --binary target/release/render-lab
   python3 tools/video/check.py --investigation godot/ssr --lang zh-CN --json
   ```

2. 定义带 source 的规范化 cue/key 数据；包含 paragraph／beat 归属、按源码顺序的动作、未解析的 Length 和资源引用。保留符号时长，不为运行校验伪造音频和帧数。
3. 从 Rust `Timeline::validate` 中抽出可复用的“值／引用／motion 形状”检查，通过一个只读 JSON 接口给 Python 使用。原运行时继续调用同一套规则。检查不执行 script 动作或播放效果。
4. 将不依赖测量的检查安排到 TTS 前；实测时间线生成后，在 `--prepare-only` 返回、渲染或写成功标记之前，再执行完整 Timeline 校验。
5. 结构化诊断至少包含 code、severity、file、line、column、beat、字段和 message；只有拿得到精确位置时才填精确范围。标题、span 或 helper 展开只拿得到邻近位置时，记录回退说明。
6. `presentation.where()` 保留已有完整位置；cue 自身优先，不把所有错误归到段落第一行。Timeline 可用可选字段或 sidecar 存完整 provenance，现有 `source` beat ID 兼容保留。

验收：在没有视频 Python 环境、配音缓存和 GPU 的条件下，使用已构建 host 完成真实故事的语义检查；拼错参数、错误动作实参、错误 imageGraph、错误 motion 形状在调用 synthesize 前失败；最终 prepare 不能输出未验证却宣称成功的时间线；原运行时仍拒绝直接提供的非法 timeline。位置从 source 到最终错误不丢失；实际 IR 无法提供的位置作为单独上游需求记录，不据此阻塞其他工作。

### PR 2 — 补齐标题、ID、数值和翻译结构检查（P0）

覆盖 RL12–RL16。

1. 在 list 变 dict 之前做 chapter／beat ID 和标题属性的重复检查；翻译也在对应转换前检查。
2. 章节编号检查整数性、正值和唯一性，取消静默截断。检查空故事和空章节。
3. 为 video／chapter／beat 定义内建属性 schema；允许明确登记的元数据扩展；拼错内建属性给候选项。
4. 检查 fps、尺寸、captionWidth、chapter target 和 pacing 的类型、finite 及约定值域。时间相关字段分别定义正值或非负规则；零时长的既有意图先确认，再决定规则。
5. 保留已有 focus、source、listing、beat narration 和翻译对应检查；将用于用户输入验证的 Python `assert` 改成显式诊断，避免优化模式关闭验证。

验收：重复 chapter ID、重复翻译 beat ID、`number=1.7`、负 target、NaN／Infinity、零 fps、错误尺寸、标题字段拼写错误均在检查阶段失败并定位；合法元数据、现有故事和翻译通过。重复项诊断包含前后两个声明的位置。baseline＋override 的 cue 属性顺序语义不因“重复标题属性”规则受到影响。

### PR 3 — 引用命名空间和高亮校验（P0）

覆盖 RL06–RL09。

1. 先按用途建目录：封闭 enum、静态资源／listing／pass／tag、story 声明的 image／arrow／label 实例名。开放实例名不能一律按预置资源表拒绝。
2. 增加 pass 高亮、指定 listing 的 tag 高亮等有上下文的入口。接口名称在 vocabulary 冲突检查后确定；旧 `highlight` 暂时保留。
3. 验证旧 highlight 时重放当前 demo／panel／listing／record 等上下文，避免拿 pass 和 tag 的总并集放过错误。对继承状态也要计算实际上下文。清空列表合法。
4. 统一校验 String 函数、生成常量和 raw attr 的最终结果；检查 imageGraph 的 node/resource、action 的名称和实参、参数的存在和维度。复用 PR 1 的 Rust 检查，不复制语义。
5. 对 `stop`／hide 区分“名称不存在”和“合法名称当前不活跃”；后者可以是合法幂等清理。是否拒绝 story 前向实例引用需明确并测试。

验收：`ssrTrcae`、错误 listing tag、错误 panel 值、错误 raw 参数名在 TTS 前失败；同名 tag 位于其他 listing 时不能误通过；动态图片实例、合法 PNG 资源、幂等清理和空清除仍工作。错误能指出目标类别和最接近的合法名字。

### PR 4 — motion、range、状态和时间契约（P0/P1）

覆盖 RL04、RL11、RL17–RL21。

1. 同 key 的 motion 重复 channel 先做现有 corpus 审计，再默认报错；保留跨 key 正常接管。已有 set 与 animate 冲突检查保持。
2. 完整前移 orbit start／axis、path 非空／维度、period 和其他不依赖测量的检查；对负时间和非有限数给明确规则。
3. 文档写明 `sentence` 实际是 paragraph 的排期区间，列出发音结束、下一段开始、beat 结束和 hold 的关系。需要新的 speech-end 范围时单独新增，兼容旧 Length。
4. explain 显示每个 channel 的写入顺序和接管。嵌套／重叠 range 初期用 lint 提示，不全面禁止已有编排。
5. 把 `opening` 与 demo 切换时的 reset／carry 行为写成表；添加显式、可组合的 reset 配方。先保留当前默认行为，任何行为变化单独审阅。
6. 实测时长到手后、渲染前生成预算报告；对超预算的 warning／error 策略沿用或明确配置现有容差。
7. 翻译错误显示同一 beat 两边的句段摘要和行号。稳定 sentence ID 留待真实编辑需求验证后再提独立方案。

验收：互不重叠的多 motion key 结果不变；重复 channel 给明确诊断；正常接管不误报；word onset、range 起止、sentence 的 gap、末句 tail／hold、预算分配有针对性测试。状态配方的迁移不改变代表性帧的 camera、params、panel、callout、formula、images 和 notes。

PR 4 分为四个阶段，各自一个提交（2026-11-05）：

- **4a 检查**（已完成，5dcac21）：同 key 重复 motion channel 的语料审计与默认报错（第 1 项）；不依赖测量的 motion 形状、有限数和负时长检查前移（第 2 项）。
- **4b 契约文档与配方**（已完成）：`sentence` 区间与 reset／carry 表写入 `tools/video/README.md`（第 3、5 项）；可组合的显式 reset 配方加入 `render_lab.lam`，默认行为不变。实际实现：README 的 “Where `sentence` and `beat` end” 与 “What a key resets and carries”；`render_lab.lam` 的 `narrationCleared`（清除 callout、inset 和公式卡，`formula=off`）。
- **4c 报告**（已完成：`timeline.budget_rows` 由 build 与 verify 共用，超出 10% 容差时在写出时间线前失败；`zh.alignment` 列出两侧句段与行号）：实测音频后、渲染前的预算报告（第 6 项）；翻译对齐错误显示两侧句段与行号（第 7 项）。
- **4d channel 写入**（已完成，随 PR 6）：explain 中的写入顺序与接管（第 4 项），与 PR 6 的 explain 一起实现。

### PR 5 — 精简高频写法，并试改真实章节（P1）

进度（2026-11-05）：试点已完成。`render_lab.lam` 增加 `glide`（单 channel 到单值，整个 cue 属性）、`pose`／`aim`（具名相机点）、`cameraPathOver`／`cameraTurnOver`（任意 Length）；SSR storyboard 改写 50 处 glide 与 32 段相机路径，各版本准备产物经 compare 相同。已推广到 SDFGI、VoxelGI、O3DE 的 storyboard（compare 相同）。PR 5 完成。

覆盖 RL01–RL04。

1. 用 SSR 章节中真实出现的三种场景做试点：按句单 channel 运动、相机路径、带基线的开场镜头。先记录旧输出，再设计薄封装。
2. 增加一次关键帧／一组运动的便捷形式，隐藏没有业务含义的列表包装与固定 `cue "key"`。
3. 增加命名相机 pose 或逐行 waypoint 宏；增加可指定 Length 的 cameraPath／cameraTurn 形式，默认兼容原 beat。
4. 简单值组合保持普通函数。只有需要让作者省去列表、重复语法或绑定结构时用模式宏。不要为每个属性再造一套同义函数。
5. `render_lab` 只放跨调查通用部分；`baseRoom`、特定相机配方等留在 story 库。不要把活动动画数据搬进 codeBlock；那只会成为显示内容。
6. 更新 `tools/video/README.md` 和 `.agents/skills/render-lab-lamlet/SKILL.md`，用试点后的实际代码给出推荐写法及低层退路。

验收：常见“按句单 channel 运动”无需作者手写 motion 列表和单元素 waypoint 列表；每个相机点可单独一行编辑，坐标含义清楚。真实章节重写后，旁白文本、cue 顺序、帧、插值参数和可见结果相等；再逐步推广到其他故事。先比较编辑步骤和容易出错的位置，不只比较字符数。

### PR 6 — beat explain 与局部预览（P1）

进度（2026-11-05）：已完成，含 PR 4d。`check.py --explain BEAT`（`tools/video/explain.py`）列出句段、按源码顺序的 cue、展开属性、引用名、写入的 channel 与接管来源；已准备时附帧与秒，否则注明未测量。`build.py --preview BEAT` 渲染该 beat 的起止、每个 cue 及其前一帧、motion／blend 终点和中点，并写 `index.json`。同词重复 set 的 lint 在现有公开故事中无告警。

覆盖 RL18、RL20–RL25。

建议为 `check.py` 增加 explain 或单独提供 explain 工具；下列是目标接口，不是现有命令：

```sh
python3 tools/video/check.py --investigation godot/ssr --explain --beat c1-adds
```

explain 应包含：原文与 spoken text、cue 源位置、展开后属性、资源引用、所写 channel、继承／覆盖来源、Length 和范围边界，以及 warning。没有实测音频时保留符号时间，明确显示“未测量”；已有缓存时可额外列出实际秒数和帧，缓存缺失不自动合成。

局部预览按 chapter／beat／cue 选择，复用现有渲染命令。支持 cue 前后、轨迹端点、可选均匀采样和短片段；保留已有 midpoint 总览。若缺少音频缓存，明确选择是否先准备所需内容。状态必须从完整时间线正确求值，不能把单个 beat 抽出后丢失之前的继承状态。

验收：作者无需计算帧号就能检查指定 beat；错误高亮、瞬时 cue 和运动端点能进入快照；只跑一次 offscreen GPU 作业，不打开用户桌面窗口。无缓存 explain 不合成音频，也不编造精确时长。

### PR 7 — schema、发现和回归整合（P2，部分可提前）

进度（2026-11-05）：
- **7a（已完成）**：`render-lab story schema`（`lab/src/script/keys.rs` `schema`）给出每个 key 的 attribute、函数名，值的类型、单位、范围、默认值与说明；`omitted` 列出被 vocabulary 省略的 option 与 key（不是合法 Lamlet 名、名字冲突、函数名被占用），并给出仍可使用的 `{attr "…" "…"}` 写法。`story keys` 输出不变。（第 1、2 项）
- **7b（待做）**：motion 由 String 中的 JSON 改为结构化 Data（第 4 项）：`render_lab.lam` 的 `Motion` 改为按 kind 的 variant，各包装函数相应改写，`markup.py` 接受 Data；以 compare 验收。
- **待上游**：vocabulary 的封闭选项集合（lamlet-present 的 `"closed": true`）需要 lamlet-present 发布并在 `lab/Cargo.toml` 中重新固定版本，由用户决定；在此之前由 `story check`（`lab/src/story_refs.rs`）在语音合成前检查这些名字。（原 typed-content 计划第 3 阶段；该计划其余部分已完成并删除。）
- 不做：第 3 项，Python `markup.TYPED`（Python 自行解析的字段）与 Rust `keys::TYPED`（render_lab 类型化的属性）含义不同，合并无益；第 5 项，仓库没有 CI，回归仍以 `compare.py` 和本地检查执行。

覆盖 RL05、RL26，并固定前六项的验收基础。

1. 扩充 `story keys` 的 schema：原 key、Lamlet 名、类型／维度、单位、选项或引用类别、默认值、检查方式、推荐入口和示例。尽量从已有注册信息生成。
2. 对命名冲突、被省略的 key／option 输出可查询报告；仍能使用 attr 时给出明确映射，不静默留给作者猜。
3. Python `Fields` 应保留完整已知 key／option 信息；集中维护 typed 字段契约，减少 Rust、Lamlet 和 Python 名单漂移。
4. 新的结构化属性优先利用现有 Data 通路。motion 的手工 JSON、名字列表的 CSV 等可分批改为结构化数据；明确版本和兼容读取，避免一次重写全部格式。
5. 把 preflight 加入 CI；将 `compare.py`／`regression.py` 作为行为保持改造的验收工具。诊断 metadata 若进入输出，需显式版本化或在单独 sidecar 比较，不能粗暴忽略语义差异。

验收：新增一个字段不需要在多处手工重复列举；合法选项、帮助与运行时规则一致；冲突名称可发现；旧故事兼容。回归能分别指出字幕／语音、时间线／状态、诊断 metadata 的变化。

## 执行顺序与里程碑

| 里程碑 | 内容 | 完成标准 |
|---|---|---|
| M1：先能便宜地发现错误 | PR 1；PR 2 可并行推进 | 独立无音频检查可用，结构错误与已有宿主语义错误前移，prepare 必须通过最终验证。 |
| M2：消除静默引用错误 | PR 3；PR 4 的冲突和数值检查 | 高亮和重复 channel 有明确诊断；已有合法接管、动态实例和清理行为保留。 |
| M3：降低编辑负担 | PR 5；PR 4 的文档与状态配方 | 真实章节试点通过，常见写法明显减少结构性编辑。 |
| M4：缩短局部反馈循环 | PR 6 | 按 beat explain 和预览，精确区分符号／实测时间。 |
| M5：降低长期维护成本 | PR 7 | schema 和发现能力统一，回归持续运行。 |

不必等大规模 schema 重构才修 typo、ID 覆盖和晚报错。高层 API 也应在 preflight 与基线可用之后推广。

## 验证与迁移要求

1. 保存修改前的 prepared outputs；语音和语义保持的改造使用现有 compare。可用 `regression.py` 覆盖公开 corpus：Godot SSR、SDFGI、VoxelGI 和 O3DE Lighting 的现有语言版本。先备齐缓存和匹配 host／package libraries。
2. 负例测试覆盖：高亮 typo／错误 namespace、错误 raw attr、错误 String 选项、重复 chapter／translation ID、分数章节号、非法数值、错误 action／image 引用、重复 motion channel。测试错误发生在 synthesize 之前，而不只是最终退出非零。
3. 计时测试使用受控的测量数据，覆盖 point、range、sentence／beat、gap／tail／hold、同帧 key 顺序；无需启动 TTS。
4. 纯语法或 helper 改造要求旧／新旁白、caption、timeline 语义和 prepared artifacts 相等。诊断附加数据单独比较；不要用“更新 golden”掩盖时序变化。
5. 改动 `lab/` 或 `packages/render_lab/` 时，按仓库 AGENTS 执行对应构建、测试和 frame identity；修改 storyboard 形式／reader 时，执行 `tools/video/compare.py` 和相关视频测试。GPU 任务一次一个，检查 offscreen 输出。
6. 若测试发现旧故事依赖隐式覆盖或错误引用，将“规则变严格”和“修正受影响内容”写清楚；不要在改进工具的同时偷偷重排故事或调整镜头参数。
7. 每个 PR 只承诺它实际验证的层次：语义检查不等于精确计时检查，精确计时检查不等于 GPU 画面验证。现有 consumer regression 检查准备结果，不替代视觉验证。

## 本次复核记录与限制

原审计未修改 Render Lab，也未构建 host、运行 TTS 或 GPU。进行了源码追踪，并用独立 Python 调用和 mocked Lamlet IR 复现解析／board 验证边界。下方保留当时的复现结果；原临时 JSON 附件现已不可用。本次重新提供计划，没有重新运行审计。

复现结果：重复 chapter ID 通过 board validate；`number=1.7` 得到 1；负 target 被接受；header 接受 `fps=0`、负 lead 和拼错的 `gaap`；翻译重复 beat 在字典化时只剩一项；Python key 读取接受非法 panel 值、未知参数名和错误 highlight；NaN 被数值读取接受；负 motion seconds 保留；`where` 丢弃 column。对照例确认错误 cue 类型和错误 focus 会被拒绝。

这些实验说明早期检查的缺口，不表示所有值最终都能被 Rust timeline 或 renderer 接受。motion 重复 channel 的覆盖、晚期检查位置和状态继承由源码确认，尚未用真实视频重现。以上验收测试是后续实施要求，不是本次已通过的检查。
