# godot_sdfgi: the source index's text in Chinese (Project Fluent; lab/src/sources.rs). The English is the source
# index's own, written by src/…/source_index.py; a key names what it translates: `entry-<key>-inspect`,
# `entry-<key>-detail-<n>` (the entry's n-th detail, from 1), `chapter-<n>-title`, and `mapping-<lab>-engine-name` (the lab's name, other than letters, digits, `_`, `-` as `-`),
# `-exact`, `-lab-layout`.

entry-frame-order-inspect = SDFGI 的工作从 Forward+ 一帧的开头开始：_update_sdfgi 在其他一切准备之前体素化本帧的 dirty region 并烘焙静态光。同一函数随后调用 update_cascades、pre_process_gi（动态光）和 update_light，之后还有 store_probes 与 process_gi。
entry-frame-order-detail-1 = 级联 UBO 在 _update_sdfgi 之后才在此写入（发现 F10）
entry-frame-order-detail-2 = 之前是 pre_process_gi（动态光、SDFGIData UBO），然后是对 solid cell 的光照渲染阶段
entry-frame-order-detail-3 = update_probes 由此运行，与深度预处理阶段并行
entry-frame-order-detail-4 = 八面体贴图每帧存储，随后 gi.process_gi 运行屏幕空间渲染阶段
entry-sdfgi-update-inspect = 每个视口的 SDFGI 对象：SDFGI 关闭时释放，开启时创建；级联数、最小单元格大小、历史记录大小、遮蔽标志或 y 轴缩放不同时重建（清空所有贴图，所有级联为 DIRTY_ALL）；否则运行 SDFGI::update，由它移动级联。
entry-sdfgi-update-detail-1 = 关闭或 reset 时释放对象
entry-sdfgi-update-detail-2 = history 大小表；默认索引 5 即 30
entry-sdfgi-update-detail-3 = 改变就会全部重建的设置
entry-sdfgi-update-detail-4 = 唯一会滚动级联的调用
entry-settings-inspect = Environment 的 SDFGI 属性及默认值：SDFGI 关闭，4 个级联，最小单元格 0.2 m，y 轴缩放为 75 %，遮蔽关闭，反弹反馈 0.5，读取 sky light 开启，能量 1.0，法线偏移与探针 bias 均为 1.1。
entry-settings-detail-1 = 4 个级联
entry-settings-detail-2 = 最细的 cell，单位米
entry-settings-detail-3 = 75 %，引擎把它变成 y_mult 1.5（见 y-mult-table）
entry-settings-detail-4 = 反弹反馈
entry-project-settings-inspect = 三个项目设置在 GI 对象创建时只读取一次：probe_ray_count、frames_to_converge 与 frames_to_update_lights，各自限制在枚举范围内。运行时只有 RenderingServer 的 setter（environment_set_sdfgi_ray_count 与两个帧数设置）能修改它们。
entry-project-settings-detail-1 = frames_to_converge，是 { "{" }5, 10, 15, 20, 25, 30{ "}" } 的索引
entry-project-settings-detail-2 = frames_to_update_lights，是 { "{" }1, 2, 4, 8, 16{ "}" } 的索引
entry-mobile-stub-inspect = Mobile 渲染器的 sdfgi_update 函数体为空，pending region 数量返回 0，所以 SDFGI 在那里什么也不做（设置的 setter 仍然有效）；Compatibility 渲染器有相同的空实现（gles3-stub）。
entry-mobile-stub-detail-1 = 永远没有待处理的 region
entry-gles3-stub-inspect = Compatibility（GLES3）渲染器的 SDFGI 入口同样是空实现：sdfgi_update 什么也不做，pending region 数量为 0。
entry-gles3-stub-detail-1 = 返回 0
entry-prepass-gate-inspect = 深度预处理阶段在项目设置 rendering/driver/depth_prepass/enable 开启（默认）或不透明模板强制时运行；SDFGI 从不强制它。探针更新只在这个分支内启动，所以设置关闭时探针停止更新，从代码中读出（发现 F8）。
entry-prepass-gate-detail-1 = stable 中唯一会强制深度预处理阶段的条件
entry-prepass-gate-detail-2 = 包含探针更新的分支
entry-prepass-gate-detail-3 = update_probes 从这里与深度预处理阶段并行启动
entry-prepass-call-inspect = _post_prepass_render 是 update_probes 的唯一调用者；viewport 没有 SDFGI 对象时它立即返回。
entry-prepass-call-detail-1 = 没有 SDFGI 对象就没有探针更新
entry-ray-count-table-inspect = 代码里的 ray count 表有七项，{ "{" }4, 8, 16, 32, 64, 96, 128{ "}" }；默认设置即索引 1，每个探针每帧 8 条射线，而不是编辑器中索引 1 的标签所写的 16（发现 F7，从代码中读出）。
entry-ray-count-table-detail-1 = 表由项目设置索引
entry-ray-count-hint-inspect = 编辑器对 probe_ray_count 的提示列出六项，"8 (Fastest),16,32,64,96,128 (Slowest)"，默认值为 1，所以编辑器把默认值标为 "16"、索引 0 标为 "8"；代码的表（ray-count-table）有七项、从 4 开始，因此它们实际是 8 条和 4 条射线（发现 F7）。
entry-ray-count-hint-detail-1 = 作为对照，下一个设置的提示 "5 ... 30" 与它的表一致（history 大小 5 到 30；默认索引 5 即 30）
entry-cascade-create-inspect = 创建一个 SDFGI 对象：读取 environment，把 probe_axis_count 设为 PROBE_DIVISOR + 1 = 17，并分配 scratch 与逐级联的贴图，全部清零。每个级联起始为 DIRTY_ALL，所以第一帧体素化整个网格。
entry-cascade-create-detail-1 = 每个轴 17 个探针
entry-cascade-create-detail-2 = 初始位置是取整到整个探针的相机位置
entry-cascade-create-detail-3 = 所以位置是 8 个 cell 的整数倍
entry-cascade-create-detail-4 = 每个级联起始为完全 dirty
entry-cell-size-double-inspect = 每设置完一个级联，单元格大小翻倍，所以级联 i 的单元格为 min_cell_size * 2^i（0.2、0.4、0.8、1.6 m），始终是 128 个单元格宽：盒子宽 25.6、51.2、102.4、204.8 m。
entry-cell-size-double-detail-1 = 该级联取当前大小
entry-cell-size-double-detail-2 = 第一个级联从最小值开始
entry-y-mult-table-inspect = 环境的 y 轴缩放 50 %、75 %、100 % 对应 y_mult 2.0、1.5、1.0。世界坐标 y 先乘以它再划分网格，所以默认 75 % 时体素高 cell_size / 1.5，级联的高度是宽度的三分之二：这是竖直方向的压缩，而不是“75 % 的高度”（发现 F3，从代码中读出）。
entry-y-mult-table-detail-1 = 引擎处处使用的 y_mult
entry-y-mult-table-detail-2 = 初始位置先压缩相机的 y
entry-probe-axis-count-inspect = 每个级联沿每个轴有 17 个探针：PROBE_DIVISOR 的 16 个间隔把两端都包含在内，所以探针每 8 个单元格（128 / 16）一个，探针 16 位于远端的面上；每个级联 17 x 17 x 17 = 4913 个探针。
entry-probe-axis-count-detail-1 = 探针之间相隔 128 / 16 = 8 个单元格
entry-probe-axis-count-detail-2 = 探针贴图宽 17 * 17 列
entry-scroll-update-inspect = 级联唯一移动的地方。相机所在的单元格是其位置（y 先乘 y_mult）除以单元格大小后向零截断；若它离级联位置超过 drag_margin = 4 个单元格，级联每次移动 8 个单元格（一个探针），dirty_regions 记录该轴上带符号的总量。
entry-scroll-update-detail-1 = (128 / 16) / 2 = 4 个 cell
entry-scroll-update-detail-2 = Vector3i 转换向零截断
entry-scroll-update-detail-3 = 向 +轴，距离超过 4 个 cell 时，一步是 8 个 cell
entry-scroll-update-detail-4 = 向 +轴移动得到负的 dirty 量（从末端填充）
entry-dirty-all-rule-inspect = 当 dirty 体积超过保留体积的一半时，部分滚动变为 DIRTY_ALL：单轴移动 d 个单元格时 3d > 128，即从 43 个单元格起，按 8 个单元格一步就是 48 个单元格（级联 0 为 9.6 m）。任一轴移动 128 个单元格以上则立即为 DIRTY_ALL。
entry-dirty-all-rule-detail-1 = 任一轴达到 128 个 cell 或以上
entry-dirty-all-rule-detail-2 = dirty 体积与保留体积的一半比较
entry-pending-regions-inspect = 一个 pending region 对应一个 DIRTY_ALL 的级联，或一个滚动过的级联中 dirty 量非零的每个轴，所以一个级联最多 3 个；region 按级联顺序编号。
entry-pending-regions-detail-1 = 完全重绘算一个 region
entry-pending-regions-detail-2 = 每个 dirty 轴算一个 region
entry-region-bounds-inspect = 把 region 编号变成级联、要填充的单元格范围和一个世界空间的盒子。dirty 轴上的 region 覆盖 0..dirty 个单元格（从起点）或 128 + dirty..128（从末端）；之前的轴被削去，避免角落被体素化两次。盒子是单元格范围乘单元格大小，y 再除以 y_mult。
entry-region-bounds-detail-1 = 为正：从起点填充
entry-region-bounds-detail-2 = 为负：从末端填充
entry-region-bounds-detail-3 = 削去之前轴的部分
entry-region-bounds-detail-4 = 以米计的盒子，y 乘 1 / y_mult
entry-sdfgi-cull-inspect = 场景中的每个可见实例（隐藏的实例不在 instance_data 中）都与每个 region 的盒子相交测试，没有视锥、遮挡或可见范围测试：相机背后的网格也会被体素化。几何实例满足 FLAG_USES_BAKED_LIGHT 且与相机的 layer 匹配即合格；该测试不检查 cast-shadows-only（发现 F17，从代码中读出）。
entry-sdfgi-cull-detail-1 = 几何条件
entry-sdfgi-cull-detail-2 = 实例加入该 region 的列表
entry-gi-mode-inspect = 只有 GI mode 为 Static 才会设置 cull 测试的 baked-light 标志，所以只有 Static 网格被体素化；Dynamic 让网格不进入体素但仍接收 SDFGI，Disabled 两者都不做。
entry-gi-mode-detail-1 = Static：USE_BAKED_LIGHT 为 true
entry-gi-mode-detail-2 = Dynamic：USE_BAKED_LIGHT 为 false，USE_DYNAMIC_GI 为 true
entry-static-light-cull-inspect = 可见的、烘焙模式为 Static 且盒子与某个 region 重叠的光，在该 region 的级联编号不大于光的 max_sdfgi_cascade（默认 2，所以级联 3 没有静态点光源）时，加入该级联的静态光列表。动态光不按 region 剔除。
entry-static-light-cull-detail-1 = 按级联编号存放
entry-static-light-cull-detail-2 = 每个（光，级联）一项
entry-static-lights-index-inspect = 静态光列表按级联编号存放（见 static-light-cull），但在这里以及 render_static_lights 中却按 dirty 级联列表中的位置读取：只有 dirty 级联恰为 0..n-1 时两者才一致。只有级联 2 dirty 时，位置 0 为空，该帧这盏光被丢弃（发现 F2，从代码中读出）。
entry-static-lights-index-detail-1 = 按位置 i 读取
entry-static-lights-index-detail-2 = 数组原样传下去
entry-light-bake-route-inspect = 烘焙模式为 Dynamic（默认）的非方向光保存在 scenario->dynamic_lights 中，由逐帧的动态渲染阶段读取；Static 光不在任何列表中，只通过 region 剔除到达 SDFGI。方向光在这里从不获得 bake mode，所以总是动态的。
entry-light-bake-route-detail-1 = 离开 Dynamic 的光被移出列表
entry-light-bake-route-detail-2 = Dynamic 光被列入
entry-light-bake-route-detail-3 = 修改它不会重新烘焙
entry-render-sdfgi-inspect = 把一个 region 渲染三次，每个轴 i = 0、1、2 一次：正交相机位于 region 盒子的 +轴面，沿 -轴看去，投影恰好是该盒子，渲染到空 framebuffer，每个体素一个像素。渲染阶段 mode 为 PASS_MODE_SDF，使用双面 cull 变体，所以两个面都绘制；不写任何颜色渲染目标。
entry-render-sdfgi-detail-1 = 体素化的渲染阶段 mode
entry-render-sdfgi-detail-2 = 恰好是 region 盒子的正交投影
entry-render-sdfgi-detail-3 = 另外两个轴上每个体素一个像素
entry-render-sdfgi-detail-4 = 空 framebuffer：没有颜色或深度渲染目标
entry-sdf-double-sided-inspect = 对 PASS_MODE_SDF，无论材质的 cull mode 如何，cull 变体都是双面，所以背面也会被体素化。
entry-sdf-double-sided-detail-1 = 双面
entry-sdf-fragment-inspect = 体素化的 fragment shader 写入的是 fragment 三维位置所在的体素，而不是它的像素：local_pos 是 region 盒子内缩放到 [0, 1] 的位置，grid_pos = sdf_offset + ivec3(local_pos * sdf_size)，没有 clamp。最后写入的 fragment 决定反照率。
entry-sdf-fragment-detail-1 = 由三维位置得出的体素
entry-sdf-fragment-detail-2 = 普通 store：最后写入者胜出
entry-albedo-pack-inspect = 反照率体素为 16 位（R16_UINT）：第 0 位是 solid 标志；蓝色占第 1-5 位，绿色第 6-10 位，红色第 11-15 位，每个通道为 clamp(uint(c * 31), 0, 31)，是截断而不是四舍五入。
entry-albedo-pack-detail-1 = 红，第 11-15 位
entry-albedo-pack-detail-2 = 绿，第 6-10 位
entry-albedo-pack-detail-3 = 蓝，第 1-5 位
entry-facing-mask-inspect = facing 掩码每个 fragment 一位：在 +X、+Y、+Z、-X、-Y、-Z 六个方向（第 0-5 位）中，取与插值顶点法线（转到世界空间，背面不翻转，不用法线贴图）点积最大的那个。fragment 之间用 imageAtomicOr 合并，所以体素保留任一 fragment 选中的所有方向。
entry-facing-mask-detail-1 = 点积最大的方向胜出
entry-facing-mask-detail-2 = 只有一位，不累积
entry-facing-mask-detail-3 = fragment 之间取或
entry-emission-pack-inspect = 只有高于 0.001 的自发光才会存储。每个方向的强度为 max(0, dot(normal, dir))；render_emission_aniso 把各方向占最大通道的比例存为 5 位（第 0-29 位，顺序同 facing），render_emission 把总光量存为 RGBE 8985：红取 9 位中的高 8 位，占第 0-7 位；绿 9 位，第 8-16 位；蓝取 9 位中的高 8 位，第 17-24 位；指数 5 位，第 25-29 位；第 30-31 位留给邻居位。
entry-emission-pack-detail-1 = 每个方向的强度
entry-emission-pack-detail-2 = 六个 5 位的比例
entry-emission-pack-detail-3 = RGBE 8985
entry-emission-pack-detail-4 = 光量 word
entry-render-textures-inspect = 体素化的 scratch 贴图为 128^3，由所有级联共用：render_albedo 为 R16_UINT，render_emission 与 render_emission_aniso 为 R32_UINT，render_geom_facing 为 R32_UINT；在 stable 中它没有 atomic 使用位，尽管 shader 用 imageAtomicOr 写入它。随后是遮蔽与跳跃洪泛的贴图。
entry-render-textures-detail-1 = 反照率体素的格式
entry-render-textures-detail-2 = 自发光，R32_UINT
entry-render-textures-detail-3 = facing，R32_UINT，没有 atomic 使用位
entry-render-textures-detail-4 = 接着是跳跃洪泛的 seed 贴图，RGBA8_UINT
entry-render-region-inspect = 体素化一个 region，并在级联的最后一个 region 之后运行 preprocess：级联的第一个 region 清空 scratch 贴图，随后是滚动（非 DIRTY_ALL 时）、跳跃洪泛、遮蔽与 STORE，再清空该级联的光照贴图。它还把该级联的动态光标记为 dirty。
entry-render-region-detail-1 = 级联的第一个 region 清空 scratch 贴图
entry-render-region-detail-2 = 最后一个 region 运行 preprocess
entry-render-region-detail-3 = 所以下一次光照更新刷新所有 cell
entry-jump-flood-init-inspect = 每个半尺寸单元格（64^3）以其 2 x 2 x 2 块中的一个 solid 体素作为 seed：收集其中的 solid 体素，选 closest[index % count]，其中 index 是该单元格自己 x、y、z 的奇偶位，所以相邻单元格会选不同的体素。没有 solid 的块得到 (0, 0, 0, 0)。seed 是体素的全分辨率坐标（xyz），w = 255 表示有效。
entry-jump-flood-init-detail-1 = 由奇偶位选择体素
entry-jump-flood-init-detail-2 = seed：体素自身的位置，有效
entry-jump-flood-init-detail-3 = 没有 solid 体素：未使用
entry-jump-flood-inspect = 一次跳跃洪泛渲染阶段：每个单元格查看距离为 step 的 26 个邻居，跳过 seed 的 w = 0 的，保留离自己中心（半尺寸时为 pos * 2 + 0.5）最近的 seed。MODE_JUMPFLOOD_OPTIMIZED 用 8^3 的 group 完成同样的事，每个 group 处理一个带步长的子网格，邻域放在 shared memory 中。
entry-jump-flood-detail-1 = 半尺寸单元格的中心，以全分辨率体素计
entry-jump-flood-detail-2 = 到该 cell 持有的 seed 的距离
entry-jump-flood-detail-3 = 优化变体：每个 group 一个带步长的子网格
entry-jump-flood-upscale-inspect = 回到全尺寸：solid 体素得到 (pos, 255)；其他体素取位于 pos >> 1 的半尺寸 seed，并检查该 seed 所在 2 x 2 x 2 块中的 8 个体素，保留更近的 solid 体素。
entry-jump-flood-upscale-detail-1 = 半尺寸 seed 所在块中的 8 个体素
entry-jump-flood-upscale-detail-2 = 同一块中更近的体素若为 solid 则取代它
entry-jump-flood-schedule-inspect = half_size 被硬编码为 true，所以全尺寸分支从不使用。对于 128 的级联：一次 64^3 的初始化，六次半尺寸渲染阶段，步长 32、16（普通）与 8、4、2、1（优化版；普通渲染阶段在 64 / (step / 2) 达到 8 的那一轮，即步长 16 之后结束），一次放大到 128^3，再加一次步长为 1 的全尺寸优化渲染阶段来修正半尺寸的瑕疵：共 9 次 dispatch。
entry-jump-flood-schedule-detail-1 = 当每半步的网格数达到 8 时普通渲染阶段结束
entry-jump-flood-schedule-detail-2 = 优化版半尺寸渲染阶段
entry-jump-flood-schedule-detail-3 = 最后的全尺寸渲染阶段
entry-sdf-store-inspect = STORE 把 seed 变成距离场：自己就是 seed 的 cell（solid）存 0；其他 cell 存 1 + 到其 seed 的距离（以 cell 计），然后全部除以 255 写入 R8_UNORM 贴图。没有符号，最大距离为 254 个 cell；solid cell 的面邻居存 2/255，1/255 从不被写入。
entry-sdf-store-detail-1 = solid 体素存 0
entry-sdf-store-detail-2 = 归一化到 unorm 范围
entry-sdf-store-detail-3 = 写入级联的 sdf_tex，R8_UNORM
entry-sdf-decode-inspect = 各个 march 在采样后减去一个偏移来抵消 STORE 的“1 + d”：这里的探针 march 减 1.0，shadow march（shadow-march）减 1.0，屏幕渲染阶段的反射 march（specular-trace）减 1.1，debug 视图减 1.7；求梯度的读取不减。因此 solid 纹素读出 -1（或 -1.1），solid 与空之间的线性插值使零点落在中间。场本身没有符号。
entry-sdf-decode-detail-1 = 探针 march 的命中规则
entry-solid-cell-store-inspect = 每个 solid 体素以四个 word 写入级联的列表：position（x、y、z 各 7 位，第 0-20 位）、反照率（RGB555 占第 0-14 位，六个 facing 位占第 15-20 位）、light（体素化写入的 30 位 RGBE 8985）和 light_aniso（6 x 5 位）。26 个邻居位（每个邻居对应一位，其最近 seed 就是这个单元格）拆分存放在高位：position 的第 21-31 位存邻居 11-21，反照率的第 21-31 位存 0-10，light 的第 30-31 位存 22-23，light_aniso 的第 30-31 位存 24-25。
entry-solid-cell-store-detail-1 = 偏移 n 处的体素以此单元格为最近 seed 时，第 n 位置 1
entry-solid-cell-store-detail-2 = 六个 facing 位在第 15-20 位
entry-solid-cell-store-detail-3 = 邻居位 0-10 在反照率 word 中
entry-solid-cell-store-detail-4 = 邻居位 22-23 在 light word 中
entry-solid-cell-struct-inspect = C++ 一侧声明的 solid 单元格记录：四个 uint32（position、反照率、static_light、static_light_aniso），16 字节。该 struct 并未使用，仅决定 solid_cell_buffer 的大小：每个级联容纳 128^3 的 25 % = 524,288 个单元格（shader 把后两个 word 称为 light 与 light_aniso）。
entry-solid-cell-struct-detail-1 = 第四个 word
entry-solid-cell-struct-detail-2 = 缓冲区容纳的 cell 比例
entry-occlusion-dispatch-inspect = 八次 dispatch，每个探针格点奇偶类 i 一次（offset 是 i 的各位，在级联全局探针索引为奇数的轴上取反），每次 group 数为 (9, 9, 9) 减去 offset。这段代码周围没有任何对 use_occlusion 的判断：每次级联重建都会计算遮蔽，无论是否被读取（发现 F14，从代码中读出）。
entry-occlusion-dispatch-detail-1 = 全局探针索引为奇数时，该轴上的类别取反
entry-occlusion-dispatch-detail-2 = 类别 i 选择 render_occlusion[i] 贴图
entry-occlusion-dispatch-detail-3 = 每个轴 9 - offset 个 group
entry-occlusion-shells-inspect = 每个 group 负责一个探针周围的 16^3 区域（每侧 OCCLUSION_SIZE = 8），分 22 步处理：|x| + |y| + |z| 相等的壳层从探针向外扩张再收缩，覆盖全部八个卦限，每步后设 barrier。solid 体素（facing != 0）得 0，紧邻探针的壳层得 1，其他空体素取其最多三个更靠近探针的邻居的平均值；solid 邻居贡献 0，但仍计入平均的分母，坐标平面上除外。
entry-occlusion-shells-detail-1 = solid 体素得 0
entry-occlusion-shells-detail-2 = 第一层壳得 1
entry-occlusion-shells-detail-3 = 更靠近探针的邻居的平均值
entry-occlusion-shells-detail-4 = 第三阶段：solid 体素取其靠近探针一侧空邻居的遮蔽
entry-occlusion-store-inspect = STORE 把一个体素八个类别的值打包进一个 word，每个 4 位（occ * 15，截断），类别 i 位于移位 { "{" }12, 8, 4, 0, 28, 24, 20, 16{ "}" }[i]；低 16 位写到纹素 (x, y, z + 级联 * 128)，高 16 位写到 (x + 128, y, z + 级联 * 128)。按 R4G4B4A4 查看时，R 为第 12-15 位（类别 0 或 4），G 为第 8-11 位，B 为第 4-7 位，A 为第 0-3 位。
entry-occlusion-store-detail-1 = 移位：类别 0 在第 12 位 ... 类别 7 在第 16 位
entry-occlusion-store-detail-2 = 低半部分
entry-occlusion-store-detail-3 = 高半部分沿 x 偏移 128 个纹素
entry-occlusion-store-detail-4 = 高半部分
entry-occlusion-read-inspect = 屏幕渲染阶段仅在 use_occlusion 开启（默认关闭）时读取遮蔽：类别由每个轴上 (probe_world_offset + 探针) 的奇偶决定，x 为 1、y 为 2、z 为 4；采样点被限制在距探针不超过 occlusion_clamp = 0.9375 的范围内，乘以 1/16 后重新归一化；z 位为 4 时移到 x 的后半部分；权重乘以 max(occlusion, 0.01)。
entry-occlusion-read-detail-1 = 限制在探针附近
entry-occlusion-read-detail-2 = z 位选择 x 的后半部分
entry-occlusion-read-detail-3 = 探针权重
entry-occlusion-texture-inspect = occlusion_data 为 R16_UINT，256 x 128 x (128 x 级联数)：宽度翻倍以存放第二个 16 位，深度堆叠各级联；通过 R4G4B4A4_UNORM 视图读取。
entry-occlusion-texture-detail-1 = 另外 16 位存到 x + 128
entry-occlusion-texture-detail-2 = 四位视图
entry-update-light-inspect = 动态光照渲染阶段：对每个级联，把 solid 单元格计数拷贝到 indirect dispatch 缓冲，传入 bounce_feedback 与 use_occlusion，每个 solid 单元格一个线程进行 dispatch；除非该级联的光全部为脏，每帧只处理每第 N 个单元格（process_offset 与 process_increment，N 由 frames_to_update_light 决定：1、2、4、8 或 16）。即使没有光，每个级联也会 dispatch，因为该渲染阶段还要加上自发光和已存储的静态光，并回灌反弹。
entry-update-light-detail-1 = 反弹设置在此传给 shader
entry-update-light-detail-2 = pre_process_gi 为该级联统计的光数量
entry-update-light-detail-3 = 每个 solid cell 一个线程，来自已计数的列表
entry-light-schedule-inspect = 单元格轮流刷新：frames_to_update_lights 取默认索引 2 时，表给出 N = 4，process_increment = 4，process_offset = 帧号 % 4，同一帧所有级联使用同一偏移，所以每个 solid 单元格每四帧被照亮一次。刚重建的级联（all_dynamic_lights_dirty）在当帧被整体照亮。
entry-light-schedule-detail-1 = 刚重建的级联或“1 帧”设置会处理所有单元格
entry-light-schedule-detail-2 = 偏移随帧号轮转
entry-light-schedule-detail-3 = 每 N 个 cell 处理一个
entry-light-thread-inspect = 线程 t 处理第 t * N + offset 个 solid 单元格，若超过单元格总数则返回；dispatch 并未缩小，其余线程立即退出。本帧未处理的单元格保留旧的纹素，所以移动的光最多滞后 N 帧。
entry-light-thread-detail-1 = 越过列表末尾时提前退出
entry-light-thread-detail-2 = cell 中心，尚未转为世界空间
entry-dynamic-lights-inspect = pre_process_gi 对每个级联填充最多 128 盏光：先是所有可见且不是 sky-only 的方向光（不看 bake mode、max_sdfgi_cascade 或盒子；direction.y 乘以 y_mult 后归一化），然后是级联编号不大于 max_sdfgi_cascade 且盒子与级联盒子相交的 Dynamic 点光源。该级联盒子是在 y 压缩后的空间中构造的，而光的盒子在真实空间中（发现 F9，从代码中读出）。点光源的方向没有做 y 拉伸，这对 spot light 有影响（发现 F20）。
entry-dynamic-lights-detail-1 = 跳过 sky-only 的方向光
entry-dynamic-lights-detail-2 = 级联的盒子没有 1 / y_mult
entry-dynamic-lights-detail-3 = 点光源止于 max_sdfgi_cascade
entry-dynamic-lights-detail-4 = 点光源的方向保持未拉伸（对 spot light 有影响）
entry-omni-attenuation-inspect = 位置光在距离 d、范围 r（Light3D 默认 5）、衰减指数 decay = 光的 attenuation（默认 1，而不是 2）处的衰减：(1 - (d / r)^4)^2 * d^-decay，括号内限制为不小于 0，d 下限为 0.0001。attenuation 低于 0.001 的 cell 跳过 shadow march。
entry-omni-attenuation-detail-1 = (d / r) 的四次方
entry-omni-attenuation-detail-2 = 再乘 d 的 -decay 次方
entry-omni-attenuation-detail-3 = radius 是光的范围，decay 是它的 attenuation
entry-omni-attenuation-detail-4 = 低于此值不做 shadow march
entry-shadow-march-inspect = shadow 射线从光的方向沿每个轴偏出 0.48 个单元格，再沿射线方向偏出 0.4 个单元格作为起点，然后从该单元格自己的级联向外对距离场做 sphere tracing（步长 = 采样值 * 255 - 1 个单元格）；采样值低于 0.001 个单元格即命中，在步进之前判断。离开最后一个级联视为被照亮。阴影是二值的：has_shadow 被拷贝但从未被读取，所以每盏光都投射阴影，且 march 的遮蔽值从未使用（发现 F18，从代码中读出）。
entry-shadow-march-detail-1 = 起点偏移的第二部分
entry-shadow-march-detail-2 = 从字节解码出的步长
entry-shadow-march-detail-3 = 命中规则
entry-shadow-march-detail-4 = 离开级联后剩余的光距离
entry-aniso-pack-inspect = cell 的光按面保存：对 cell 暴露的六个面中的每一个，light_accum[j] 加上 max(0, dot(face, direction)) 乘以光（没有 1/pi）。然后打包：总量以 E5B9G9R9（r | g << 9 | b << 18 | e << 27）写入 light_data，每个面占最大通道的比例以 UNORM 存储，+X +Y +Z -X 存入一张 RGBA8 贴图，-Y -Z 存入一张 RG8 贴图；这三个值被拷贝到该 cell 的每个邻居中。这些比例是相对权重，总和不为 1。
entry-aniso-pack-detail-1 = 按 cell 暴露的每个面累加的面权重
entry-aniso-pack-detail-2 = 动态渲染阶段的 E5B9G9R9 word
entry-aniso-pack-detail-3 = 第二张贴图存 -Y 与 -Z
entry-aniso-pack-detail-4 = 每个邻居得到相同的光
entry-static-pass-inspect = 静态渲染阶段只在某个级联被重建且有静态光被剔除进该级联的帧运行：对每个 dirty 级联，填充最多 1024 盏光（跳过级联编号超过其 max_sdfgi_cascade 或盒子不与该级联相交的光），然后对每个 solid 单元格以反弹 0、process_offset 0、process_increment 1 dispatch MODE_PROCESS_STATIC，没有光则跳过。它按 dirty 列表中的位置读取光（发现 F2）。
entry-static-pass-detail-1 = 最多 1024 盏静态光
entry-static-pass-detail-2 = 盒子测试
entry-static-pass-detail-3 = 按 dirty 列表中的位置 i 读取光，而不是按级联编号
entry-static-pass-detail-4 = 每个 cell 一次
entry-static-write-inspect = 静态渲染阶段不写贴图：它写回 solid cell，保留两位邻居位，其余光 word 用新的 RGBE 8985 总量替换，并重建 light_aniso。由于求和从 cell 当前的光开始，cell 最终得到旧的光加上这次剔除进来的每盏光。RGBE 8985 丢掉红色与蓝色的最低位（发现 F24）。
entry-static-write-detail-1 = 求和从 cell 当前的光开始
entry-static-write-detail-2 = RGBE 8985：红与蓝的最低位被移出
entry-static-write-detail-3 = 该 word 被替换
entry-scroll-reinject-inspect = SCROLL 把保留 cell 存储的 light word（其自发光加上之前的静态光）拷贝到已清空的 render_emission 贴图的新位置，STORE 再把 render_emission 读回列表。随后静态渲染阶段把每盏被剔除的光再次加到被保留的 cell 上，所以与旧内容和新 slab 都相交的静态光在每次滚动时都会被重复计入；DIRTY_ALL 时不会（发现 F1，从代码中读出）。
entry-scroll-reinject-detail-1 = 写入自发光贴图
entry-scroll-reinject-detail-2 = STORE 把它读回 cell 记录
entry-bounce-feedback-inspect = 仅动态渲染阶段，且 bounce_feedback 高于 0.001 时：feedback 颜色在反弹小于 1 时为反照率 * 反弹，高于 1 时为 mix(albedo, 1, bounce - 1)。对单元格周围的 8 个探针和它暴露的每个面，权重为 trilinear * max(0, dot(面, 指向 probe 的方向))（开启遮蔽时再乘以遮蔽）；该面的光是各探针在该面方向上的辐照度的加权平均，再乘以 feedback。它读取上一帧末存储的贴图，所以每次反弹需要一帧。
entry-bounce-feedback-detail-1 = 单元格以探针为单位的位置，即乘以 16 / 128
entry-bounce-feedback-detail-2 = 一个探针对一个面的权重
entry-bounce-feedback-detail-3 = 读取的辐照度贴图
entry-bounce-feedback-detail-4 = 衰减后的结果
entry-update-probes-inspect = 对每个级联，每个探针一个线程（17 x 17 列、17 行，z 切片并排）串行追踪它的射线。history_index = render_pass % history_size，且 render_pass 每次调用只加一，所以它统计的是探针更新次数，而不是帧数。ray_bias 是 probe_bias 设置（1.1 个单元格），射线数来自查表；仅在 read_sky_light 开启时才读取天空。
entry-update-probes-detail-1 = 本次更新对应的 ring 槽
entry-update-probes-detail-2 = 起点偏移，单位为 cell
entry-update-probes-detail-3 = 仅在开启时读取天空
entry-update-probes-detail-4 = 每个探针一个线程：289 x 17
entry-probe-position-inspect = 线程对应的探针：probe_cell = (pos.x % 17, pos.y, pos.x / 17)，其位置为级联的 offset 加上 probe_cell 乘以 probe_cell_size，即 128 / 16 = 8 个单元格，位于 y 拉伸后的空间；探针位于单元格坐标 0、8、...、128。
entry-probe-position-detail-1 = x 与 z 切片共用图像的一个轴
entry-probe-position-detail-2 = z 切片
entry-probe-position-detail-3 = 探针在 y 拉伸空间中的位置
entry-vogel-rays-inspect = 探针的射线是 Vogel 螺旋：序号 idx = history_index + i * history_size，总数 ray_total = history_size * ray_count（默认 240），r = sqrt(idx + 0.5) / sqrt(ray_total)，theta = idx * 黄金角 + 由世界探针索引哈希出的逐探针偏移（没有时间输入）。30 帧内序号恰好覆盖 0..239 一次。z 的符号是 idx 的奇偶性，所以当 history_size 为偶数时，同一帧的所有射线都指向同一个 z 半球：偶数帧 -z，奇数帧 +z（发现 F22，从代码中读出）。射线的 y 随后乘以 y_mult 并归一化。
entry-vogel-rays-detail-1 = z 的符号取决于序号的奇偶
entry-vogel-rays-detail-2 = 第 i 条射线在帧槽 history_index 中的序号
entry-vogel-rays-detail-3 = 逐探针的旋转
entry-vogel-rays-detail-4 = y 拉伸后的空间
entry-probe-march-inspect = 射线从探针出发，沿其主轴方向偏出 ray_bias = 1.1 个单元格（是立方体而不是球），然后从探针自己的级联向外 march：在一个级联内按存储的距离（采样值 * 255 - 1 个单元格）步进，直到某一步小于 0.05 个单元格（命中）或射线离开盒子，此时从出口点在下一个级联继续。没有迭代次数上限；离开最后一个级联即为 miss。
entry-probe-march-detail-1 = 命中规则，单位 cell
entry-probe-march-detail-2 = sphere tracing 的步进
entry-probe-march-detail-3 = 射线命中所在的级联
entry-probe-march-detail-4 = 出口点成为下一个级联中的起点
entry-probe-hit-inspect = 命中只读取射线命中所在的级联：法线是存储距离的中心差分梯度（uvw 中 epsilon 为 0.001），光来自体素光照贴图，六个面的权重是 aniso0.rgb 中的 +X +Y +Z 与 (aniso0.a, aniso1.rg) 中的 -X -Y -Z。射线的辐射量是 hit_light 乘以正向各面 max(0, n * w) 与负向各面 max(0, -n * w) 之和：没有 BRDF、余弦或距离衰减，因为反照率已包含在存储的光中。
entry-probe-hit-detail-1 = 梯度步长
entry-probe-hit-detail-2 = 存储的光
entry-probe-hit-detail-3 = 负方向各面的权重
entry-probe-hit-detail-4 = 射线的辐射量
entry-probe-sky-inspect = miss 读取天空：有天空时，射线按天空朝向的逆旋转后，在 mip level 2 读取辐亮度贴图（“因为我们通常不会发射很多射线”）并乘以 sky_energy；背景为颜色时取该颜色乘以 sky_energy；否则为黑。在 4.7.2-stable 中，探针 shader 以 texture2D 的天空绑定编译，而默认开启 texture_array_reflections 时天空辐亮度是 2D array，所以 miss 的射线读到什么，代码并未定义（发现 F25，从代码中读出；见 sky-define 与 sky-辐亮度-array）。
entry-probe-sky-detail-1 = 被编译的声明（array 版本需要 USE_RADIANCE_OCTMAP_ARRAY）
entry-probe-sky-detail-2 = 编译路径所用的 mip 2 读取
entry-probe-sky-detail-3 = 颜色背景
entry-sky-define-inspect = 当天空使用八面体 array 时，探针 shader 以 USE_OCTMAP_ARRAY 构建，但 sdfgi_integrate.glsl 判断的是 USE_RADIANCE_OCTMAP_ARRAY（forward、mobile 和 fog shader 被赋予的就是这个名字），所以 array 声明从不被编译。从代码中读出（发现 F25）；本仓库历史中 4.7.2-stable 之后的提交 d9ea5c26 将它改为 USE_RADIANCE_OCTMAP_ARRAY。
entry-sky-define-detail-1 = 添加该 define 的条件
entry-sky-radiance-array-inspect = sky_use_octmap_array（rendering/reflections/sky_reflections/texture_array_reflections，桌面端默认开启）时，天空的辐亮度是 TEXTURE_TYPE_2D_ARRAY；update_probes 把该贴图绑定到探针 shader 的 set 1 的 binding 0，而编译出的 shader 在此声明的是 texture2D。
entry-sky-radiance-array-detail-1 = array 路径
entry-sh-accum-inspect = 每条射线把 light * Y_m(direction) 加到 16 个实球谐系数上（l = 0 到 3），每个颜色通道一份，存放在每线程的 shared 数组中（顺序：0；y、z、x；五个 l = 2 项；七个 l = 3 项；z 是 m = 0 轴）。SH_SIZE != 16 的分支是死代码。
entry-sh-accum-detail-1 = 常数项
entry-sh-accum-detail-2 = 一阶项依次为 y、z、x
entry-sh-accum-detail-3 = 16 项中的最后一项
entry-sh-accum-detail-4 = 8 x 8 group 中每线程一个 SH
entry-history-ring-inspect = 每个系数以定点数存储：value = (rgb, 1) * 4 / ray_count，乘以 1024（HISTORY_BITS 10），限制在 16 位内。history 贴图以 layer 保存最近 history_size 次更新；average 贴图是 ring 的精确整数和：average 减去旧 layer，再加上新值。所以均值是滑动窗口的 box filter，而不是指数平均：从零开始 k < 30 帧后约为收敛值的 k / 30。
entry-history-ring-detail-1 = 16 位定点，缩放 1024
entry-history-ring-detail-2 = 减去最旧的槽
entry-history-ring-detail-3 = 加上最新的
entry-history-ring-detail-4 = 定点缩放
entry-history-textures-inspect = 探针历史记录为 R16G16B16A16_SINT，289 x 272 个纹素（列为 x + 17 * z，行为 y * 16 + 系数），每个级联有 history_size 个 array layer；累计平均为同样大小的 R32G32B32A32_SINT；两者在创建时都被清零，使平均值能正确工作。
entry-history-textures-detail-1 = 每个 history 槽一个 layer
entry-history-textures-detail-2 = 求和需要 32 位
entry-history-textures-detail-3 = 17 * 16 = 272 行
entry-ambient-texture-inspect = 仅在体积雾开启时，平均值的 0 号系数还会写入一张 RGBA16F 的 ambient 贴图，即 (average / history_size / 1024) * 0.88622，每个探针一个纹素，每个级联一个 layer；体积雾读取这个无方向的值。
entry-ambient-texture-detail-1 = 仅 0 号系数，且仅在有雾时
entry-sh-gain-inspect = 探针估计器存储 (4 / N) * sum(L * Y)，STORE 再乘以 4 和 l_mult = { "{" }1, 2/3 x 3, 1/4 x 5, 0 x 7{ "}" }，整条链上没有 1/pi 或 4*pi；对辐射量为 L 的均匀天空，辐照度纹素为 (4 / pi) L = 1.273 L，而 Godot 自己的 sky ambient 是 1.0 L（发现 F4，从代码中读出；代码没有说明这是否是有意的增益）。
entry-sh-gain-detail-1 = 估计器的 4 / N
entry-sh-gain-detail-2 = 卷积权重
entry-sh-gain-detail-3 = 辐照度用 l_mult 加权每一项
entry-sh-gain-detail-4 = 辐亮度不用
entry-store-probes-inspect = 只要使用 GI，每一帧（无论探针是否更新），每个八面体纹素一个线程（每个级联 289 * 6 乘 17 * 6 = 1734 x 102）都会把当前平均值重新编码进八面体贴图；为此 image_size 乘以 LIGHTPROBE_OCT_SIZE = 6。
entry-store-probes-detail-1 = 每个探针的贴图为 6 x 6
entry-store-probes-detail-2 = 每个纹素一个线程
entry-oct-store-inspect = 每个纹素的方向是 octahedron_encode(local_pos / 6)，没有加半个纹素：纹素 k 存的是 u = k / 6 的方向，尽管它的中心在 k + 0.5（发现 F5，从代码中读出）。它的辐照度与辐亮度来自同样的 16 个系数，辐照度用 l_mult 求和，辐亮度不用，各自编码为 E5B9G9R9（r | g << 9 | b << 18 | e << 27，通道限制在 [0, 65408]）写入两个 layer：辐照度在级联对应的 layer，辐亮度在级联 + max_cascades。
entry-oct-store-detail-1 = SH 求和
entry-oct-store-detail-2 = 辐照度 word
entry-oct-store-detail-3 = 负的振铃变为 0，上限为 65408
entry-oct-store-detail-4 = 辐亮度 layer
entry-oct-border-inspect = 每个 6 x 6 的贴图位于一个 8 x 8 的块中（oct_pos = 探针 * 8 + 1），带一个纹素的边框。纹素写自己；边缘纹素还把它在八面体折叠处的镜像写入边框（y = 0 写到 (5 - x, -1)，x = 0 写到 (-1, 5 - y)，依此类推）；角落的纹素写另外三个角。边框是为了内部之外的双线性采样点。
entry-oct-border-detail-1 = 每个探针一个 8 x 8 的块，贴图从 +1 开始
entry-oct-border-detail-2 = 边缘纹素的镜像（此处为 y = 0）
entry-oct-border-detail-3 = 角落复制到三个边框角
entry-lightprobe-textures-inspect = lightprobe_data 为 R32_UINT，2D array，2312 x 136 个纹素（17 * 17 * 8 乘 17 * 8：每个探针一个 8 x 8 的块），每个级联 2 个 layer，通过 E5B9G9R9_UFLOAT 视图读取：layer c 是级联 c 的辐照度，layer 级联 + c 是它的辐亮度。
entry-lightprobe-textures-detail-1 = 17 * 17 * 8 = 2312
entry-lightprobe-textures-detail-2 = shader 采样所用的 E5B9G9R9 视图
entry-oct-read-inspect = 读取端定位探针的块（x + z * 17 列，乘以 8，再加 1），用同样的八面体映射编码法线，并在块 + uv * 6 处用硬件双线性过滤采样：那是纹素的角点。由于写入端把 u = k / 6 的方向放进中心在 k + 0.5 的纹素，查找得到的大约是两个纹素各 50 % 的混合，贴图偏移了半个纹素（整张图的 1/12）；边框覆盖了多出来的范围（发现 F5，从代码中读出）。
entry-oct-read-detail-1 = z 切片沿 x 排列
entry-oct-read-detail-2 = 8 x 8 的块，贴图从 +1 开始
entry-oct-read-detail-3 = uv 在 [0, 1] 内，即八面体坐标
entry-oct-read-detail-4 = 双线性，在级联的辐照度 layer 中
entry-probe-scroll-inspect = 级联滚动时，每个目标探针读取 probe_cell - scroll 处的探针（scroll 是 dirty 量 / 8，每步一个探针）；若它在 0..16 之内，则整个历史记录（所有 layer，每个 16 行）和平均值被拷贝到 scratch 贴图，再由 SCROLL_STORE 拷回。ring 位置不变，所以每个 layer 的射线集保持对齐。DIRTY_ALL 时不滚动，探针从现有数据重新收敛。
entry-probe-scroll-detail-1 = 该探针在滚动前就存在
entry-probe-scroll-detail-2 = 拷贝到 scratch 贴图
entry-probe-scroll-detail-3 = 拷回
entry-probe-scroll-dispatch-inspect = 在 render_region 内，级联不是 DIRTY_ALL 时运行探针滚动：INTEGRATE_MODE_SCROLL、barrier、INTEGRATE_MODE_SCROLL_STORE、barrier，若 bounce_feedback 大于 0，再运行一次 INTEGRATE_MODE_STORE，使光照渲染阶段能读到移动后的探针。滚动量是 dirty 量 / 8，每步一个探针。
entry-probe-scroll-dispatch-detail-1 = 多次反弹需要贴图已存储
entry-probe-scroll-dispatch-detail-2 = 这些 dispatch 中没有射线工作
entry-stale-cascade-ubo-inspect = 级联 UBO 在这里重写，而此时 _update_sdfgi（见 frame-order）已经运行了 render_region，其中的探针 SCROLL shader 读取级联.data[].offset：在某个级联滚动的那一帧，该 shader 看到的是上一帧的 offset，所以新露出的探针采样父级时偏差约一步。这只是一个会重新收敛的初始猜测（发现 F10，从代码中读出）。
entry-stale-cascade-ubo-detail-1 = render_region（包括探针滚动）在这个调用内、更早运行
entry-parent-seed-inspect = 没有被滚动进来的探针（边缘上的新探针）用下一个更粗的级联来初始化：其世界位置被转为父级探针坐标，周围 8 个父级探针的平均值（各自除成均值）做三线性混合，重新量化，写入每个历史记录 layer，并把 average 设为该值乘以 history_size。新探针从一个粗略估计开始，而不是黑色。在最后一个级联中则直接复制现有数据。
entry-parent-seed-detail-1 = 对 8 个父级探针做三线性
entry-parent-seed-detail-2 = 父级的均值
entry-parent-seed-detail-3 = average 保存整个 ring
entry-parent-seed-detail-4 = 最后一个级联没有父级
entry-process-gi-inspect = 对每个像素：由深度重建 view 空间位置，从 normal-粗糙度 buffer 读取法线与粗糙度（法线 xyz * 2 - 1；粗糙度大于 0.5 时折回，再除以 127/255），并用 cam_transform 把位置、view 与法线旋转到世界朝向，相机位于原点；反射方向为 reflect(-view, normal)。随后 sdfgi_process 把位置、法线和反射的 y 乘以 y_mult 并重新归一化。判断 normal.length() > 0.5 恒为真，因为 GLSL 中 vec3 的 length() 为 3，所以天空像素也会被处理（从代码中读出）。
entry-process-gi-detail-1 = 法线解码
entry-process-gi-detail-2 = 粗糙度解码
entry-process-gi-detail-3 = 恒为真：vec3 的 length() 为 3
entry-process-gi-detail-4 = y_mult 在 sdfgi_process 内应用
entry-process-gi-dispatch-inspect = 屏幕 GI 渲染阶段：最多两个 view；创建 ambient 与 reflection 缓冲（R16G16B16A16_SFLOAT，渲染尺寸，或 use_half_resolution 时每轴一半）并对它们 dispatch compute shader。仅在使用 SDFGI 或 VoxelGI 时调用，而 SDFGI 只有在启用且 viewport debug 模式不是 Unshaded 时才算在使用。
entry-process-gi-dispatch-detail-1 = 最多两个 view
entry-process-gi-dispatch-detail-2 = ambient 缓冲
entry-process-gi-dispatch-detail-3 = 半分辨率 dispatch
entry-cascade-select-inspect = 包含该像素的最细级联：cascade_pos 是该点相对级联角点、以探针为单位的位置；选取第一个各分量都在 [0, 16) 内的级联 i。若没有级联包含它，ambient 与 reflection 为 vec4(0)，alpha 为 0，不透明 shader 保留其 sky ambient。
entry-cascade-select-detail-1 = 在 [0, 16) 之外：尝试下一个级联
entry-cascade-select-detail-2 = 没有级联包含它：alpha 为 0
entry-cascade-blend-inspect = 靠近边缘时像素与下一级联混合：blend_from = 16 / 2 - 2.5 = 5.5，blend_to = 7.5 个探针单位，len 是相机相对位置（以探针为单位）的最大分量，blend = smoothstep(5.5, 7.5, len)。最细级联与下一个级联的结果各自完整计算（16 次探针读取）后混合；在最后一个级联中 alpha 变为 1 - blend，渐变到天空。
entry-cascade-blend-detail-1 = 过渡带宽 2 个探针单位
entry-cascade-blend-detail-2 = 最大分量（L-infinity 范数）
entry-cascade-blend-detail-3 = 混合权重
entry-cascade-blend-detail-4 = 最后一个级联渐变到天空
entry-probe-weights-inspect = 先沿法线把采样点从表面推开 normal_bias（1.1 个单元格，即 0.1375 个探针单位）再选定格子；然后 8 个探针各得到 trilinear 权重乘以 max(0.005, dot(法线, 指向 probe 的方向))，所以位于表面背后的探针权重约低 200 倍；启用时再乘以 max(occlusion, 0.01)。每个探针的辐照度沿法线方向读取（乘以 exposure_normalization），总和除以总权重。
entry-probe-weights-detail-1 = 法线偏移
entry-probe-weights-detail-2 = 每个轴上的帐篷权重
entry-probe-weights-detail-3 = 加权和
entry-probe-weights-detail-4 = 除以总权重
entry-specular-probes-inspect = 来自探针的高光：粗糙度 < 0.99 时沿反射方向读取辐亮度 layer（级联 + max_cascades）；粗糙度大于 0.2 时沿同一方向混入辐照度 layer，混合量为 (粗糙度 - 0.2) * 1.25，在粗糙度为 1.0 时达到 1。
entry-specular-probes-detail-1 = 辐亮度读取
entry-specular-probes-detail-2 = 辐照度混合从 0.2 开始
entry-specular-trace-inspect = 粗糙度低于 0.2 时，屏幕渲染阶段还会让反射在各级联的距离体与光照体中 march（每个级联半径 60 个单元格，起点偏移 (1 + base_blend) * 1.1 加上法线乘 1.4）。每一轮走一步 sample * 255 - 1.1 个单元格；采样值低于 softness（0.2 + min(1, 5 r) * 4 个单元格）时加入 light * 0.5 * alpha，其中 alpha = 1 - d / softness。结果与探针辐亮度混合：reflection alpha = alpha * (1 - b) + b，其中 b = min(1, 5 r)。没有迭代计数，步长最多负一次，且是最后一步。
entry-specular-trace-detail-1 = 步长，偏移为 1.1
entry-specular-trace-detail-2 = 光减半
entry-specular-trace-detail-3 = 步进是无条件的
entry-specular-trace-detail-4 = 与探针的混合
entry-gi-energy-inspect = 环境的 SDFGI energy 在屏幕渲染阶段末尾缩放 ambient 与 reflection，而这只有不透明表面会读取；透明物体的 forward 路径从不读取它（发现 F11，从代码中读出）。
entry-gi-energy-detail-1 = reflection 同样如此
entry-gi-compose-inspect = 不透明 shader 按 alpha 把屏幕渲染阶段的缓冲混入自己的 sky ambient 与反射，所以 SDFGI 没有级联的地方（alpha 为 0）保留天空。随后累加 reflection 探针，ambient 乘以环境光遮蔽（包括 SSAO）、反照率和 1 - 金属度，再与自发光和直接光相加；这条链上没有 pi 或 1/pi。
entry-gi-compose-detail-1 = 按 alpha 混合的反射
entry-gi-compose-detail-2 = 环境光遮蔽
entry-gi-compose-detail-3 = 反照率
entry-gi-compose-detail-4 = 金属表面没有漫反射
entry-forward-gi-inspect = 使用 SDFGI 的透明表面在自己的 fragment shader 中重复同样的查找，权重、法线偏移与混合带相同，但从不 trace：use_specular 恒为 true，高光始终来自探针贴图，即使粗糙度为 0，阈值也不同（辐亮度在 0.99 以下；仅在 0.5 以上才混入辐照度，量为 (粗糙度 - 0.5) * 2）。环境的 SDFGI 能量在这里从不应用（发现 F11，从代码中读出：该字段在本文件和 forward shader 中都没有被读取）。
entry-forward-gi-detail-1 = 辐照度仅在 0.5 以上（屏幕渲染阶段用 0.2）
entry-forward-gi-detail-2 = 相同的权重
entry-forward-gi-detail-3 = 相同的混合带
entry-forward-gi-scene-inspect = 在 forward 路径中，use_specular 被硬编码为 true，所以从不进行反射 march；在最后一个级联，shader 自己向 sky ambient 混合（diffuse = mix(diffuse, ambient_light, blend)）；所有级联之外什么也不写，保留 sky ambient。它也像屏幕渲染阶段一样对位置、法线和反射应用 y_mult。
entry-forward-gi-scene-detail-1 = y_mult，与屏幕渲染阶段相同
entry-forward-gi-scene-detail-2 = 最后一个级联渐变到 sky ambient
entry-fog-sdfgi-inspect = 体积雾读取无方向的 ambient 贴图，而不是八面体贴图：对每个 froxel，取包含该世界位置的最细级联，8 个探针使用 trilinear 权重（开启 use_occlusion 时再乘以遮蔽；没有法线项或偏移，没有级联混合），用 texelFetch 读取，只用第一个级联；total_light += ambient * gi_inject。y_mult 在 struct 中声明（第 132 行）却从未被读取，而 UBO 中的级联位置是 y 拉伸的，所以 froxel 高于相机的高度被当作 dy 查找，而级联期望的是默认 y 轴缩放下的 1.5 dy（发现 F6，从代码中读出）。
entry-fog-sdfgi-detail-1 = 世界位置上没有应用 y_mult
entry-fog-sdfgi-detail-2 = 仅 trilinear
entry-fog-sdfgi-detail-3 = 对雾的贡献
entry-fog-sdfgi-detail-4 = 在雾 shader 的 SDFGIData 副本中声明，从未被读取
entry-fog-sdfgi-enable-inspect = 当 volumetric_fog_gi_inject 大于 0.0001、SDFGI 已启用且 viewport 有 SDFGI 对象时，雾使用 SDFGI；它绑定 SDFGIData UBO、ambient 贴图和 occlusion 贴图。
entry-fog-sdfgi-enable-detail-1 = ambient 贴图
entry-fog-sdfgi-enable-detail-2 = occlusion 贴图
entry-cascade-ubo-inspect = 每个级联的 UBO 为 48 字节：float offset[3]（字节 0-11）、float to_cell（12-15）、int32 probe_offset[3]（16-27）、uint pad（28-31）和 float pad2[4]（32-47）；共上传 8 个，384 字节。
entry-cascade-ubo-detail-1 = 最后 16 字节
entry-update-cascades-inspect = 填充级联 UBO：offset = (级联位置 - 64) 个单元格乘以单元格大小，位于 y 压缩后的空间；to_cell = 1 / cell_size；probe_offset = 位置 / 8。这个 8 项数组是未初始化的栈局部变量，所以不小于级联数量的槽位里是垃圾数据。
entry-update-cascades-detail-1 = 每米的 cell 数
entry-update-cascades-detail-2 = 以探针为单位的位置
entry-update-cascades-detail-3 = 上传全部 8 个槽位
entry-sdfgi-data-ubo-inspect = 屏幕渲染阶段的 UBO，496 字节：112 字节的头部（grid_size[3] 与 max_cascades；use_occlusion、probe_axis_size、probe_to_uvw、normal_bias；lightprobe_tex_pixel_size[3] 与 energy；lightprobe_uv_offset[3] 与 y_mult；occlusion_clamp[3]、occlusion_renormalize[3]、cascade_probe_size[3]，各补齐到 16 字节），然后是 8 个各 48 字节的 ProbeCascadeData：position[3] 与 to_probe，probe_world_offset[3] 与 to_cell，pad[3] 与 exposure_normalization。
entry-sdfgi-data-ubo-detail-1 = 每个级联 48 字节的最后一个 word
entry-sdfgi-data-ubo-detail-2 = 从第 112 字节起的 8 个级联
entry-sdfgi-data-fill-inspect = pre_process_gi 每帧写入 SDFGIData UBO：grid_size 128，probe_axis_size 17，cascade_probe_size 16，probe_to_uvw 1/16，normal_bias = 1.1 / 128 * 16 = 0.1375（以探针为单位），occlusion_clamp = (8 - 0.5) / 8 = 0.9375，每个级联还有相对相机的 position、to_probe = 1 / (8 个单元格)、probe_world_offset = 位置 / 8，以及 exposure_normalization = 当前 / 烘焙时。probe_bias 不在这个 UBO 中，它是 push constant。
entry-sdfgi-data-fill-detail-1 = 由每个探针间隔的体素数得出的限制值
entry-sdfgi-data-fill-detail-2 = 位置相对于相机
entry-sdfgi-data-fill-detail-3 = 1 / (8 个 cell)，以米计
entry-sdfgi-data-fill-detail-4 = 当前曝光除以烘焙时的曝光
entry-can-sdfgi-inspect = 接收与体素化是分开的：几何实例在没有 lightmap、没有 VoxelGI 且 baked-light 或 dynamic-GI 标志任一为真时可以接收 SDFGI，所以 Static 与 Dynamic 网格都接收，Disabled 不接收；有 lightmap 或 VoxelGI 的几何体仍会被体素化。
entry-can-sdfgi-detail-1 = 实例接收 SDFGI
entry-can-sdfgi-detail-2 = 绘制时的标志还要求 SDFGI 正在使用
entry-cull-sdfgi-call-inspect = 剔除步骤首先用相机位置请求渲染器更新 SDFGI（反射探针的渲染除外）；它随后报告的 region 被转成 cull.sdfgi，每个实例都要对它们测试（sdfgi-cull）。
entry-cull-sdfgi-call-detail-1 = region 盒子来自渲染器
entry-cull-sdfgi-call-detail-2 = 按顺序记录互不相同的 dirty 级联
entry-process-voxel-inspect = light 渲染阶段读取的 solid 单元格：四个 uint（position、反照率、light、light_aniso），16 字节，26 个邻居位分散在四个 word 的高位。它对反照率的注释说 facing 位是第 16-21 位，但读写它们的代码用的是第 15-20 位（从移位 15 起的 6 位）。
entry-process-voxel-detail-1 = 反照率 word 上的注释
entry-process-voxel-detail-2 = 代码：facing 位于第 15-20 位
chapter-1-title = SDFGI 在哪里运行
chapter-2-title = 跟随相机移动的级联
chapter-3-title = 体素化内容与参与光照的光源
chapter-4-title = 体素化
chapter-5-title = 距离场
chapter-6-title = 遮蔽
chapter-7-title = 体素中的光照
chapter-8-title = 探针追踪距离场
chapter-9-title = 从系数生成贴图
chapter-10-title = 像素着色
chapter-11-title = 再看一遍这一帧
mapping-SdfgiAlbedo-engine-name = 反照率体素（render_albedo，R16_UINT）
mapping-SdfgiAlbedo-exact = R16_UINT，每个 voxel 一个 texel：第 0 位为 solid 标志；第 1-5 位蓝、第 6-10 位绿、第 11-15 位红，每个通道为 clamp(uint(c * 31), 0, 31)（截断）。
mapping-SdfgiAlbedo-lab-layout = 实验室 CPU 模型（godot/sdfgi）按相同位布局打包。
mapping-SdfgiEmission-engine-name = 自发光体素（render_emission，R32_UINT，RGBE 8985）及各面的权重（render_emission_aniso）
mapping-SdfgiEmission-exact = render_emission：红取 9 位中的高 8 位，占第 0-7 位；绿 9 位，第 8-16 位；蓝取 9 位中的高 8 位，第 17-24 位；指数 5 位，第 25-29 位；第 30-31 位空闲。render_emission_aniso：六个 5 位的比例，+X 在第 0-4 位，+Y 5-9，+Z 10-14，-X 15-19，-Y 20-24，-Z 25-29，第 30-31 位空闲。
mapping-SdfgiEmission-lab-layout = 实验室 CPU 模型（godot/sdfgi）按相同位布局打包。
mapping-SdfgiSolidCell-engine-name = 级联列表中的一个实心单元（着色器中对应 ProcessVoxel，gi.h 中对应 Cascade::SolidCell）
mapping-SdfgiSolidCell-exact = 四个 uint32，16 字节。position：x 为第 0-6 位，y 为 7-13，z 为 14-20，邻居位 11-21 存放在第 21-31 位。albedo：蓝第 0-4 位，绿 5-9，红 10-14（RGB555），facing 位第 15-20 位，邻居位 0-10 存放在第 21-31 位。light：RGBE 8985 占第 0-29 位，邻居位 22-23 在第 30-31 位。light_aniso：六个 5 位的比例占第 0-29 位，邻居位 24-25 在第 30-31 位。邻居位 n（0 到 25，按偏移 (-1,-1,-1) 到 (1,1,1) 的顺序，不含中心）在该偏移处的 voxel 以此 cell 为最近 seed 时置位。
mapping-SdfgiSolidCell-lab-layout = 实验室 CPU 模型（godot/sdfgi）按相同位布局打包。
mapping-SdfgiCascade-engine-name = 级联 UBO 条目（Cascade::UBO，由 update_cascades 写入）
mapping-SdfgiCascade-exact = 48 字节：float offset[3] 位于 0-11（cascade 的最小角点，(position - 64) 个 cell 乘以 cell 大小，y 压缩），float to_cell 位于 12-15，int32 probe_offset[3] 位于 16-27（position / 8），uint pad 位于 28-31，float pad2[4] 位于 32-47。共八项，384 字节。
mapping-SdfgiCascade-lab-layout = 实验室 CPU 模型（godot/sdfgi）按相同位布局打包。
mapping-SdfgiData-engine-name = 屏幕渲染阶段的 UBO（SDFGIData，由 pre_process_gi 写入）
mapping-SdfgiData-exact = 496 字节。头部：grid_size[3] 位于 0-11，max_cascades 为 12；use_occlusion 16，probe_axis_size 20，probe_to_uvw 24，normal_bias 28；lightprobe_tex_pixel_size[3] 32-43，energy 44；lightprobe_uv_offset[3] 48-59，y_mult 60；occlusion_clamp[3] 64-75，pad 76；occlusion_renormalize[3] 80-91，pad 92；cascade_probe_size[3] 96-107，pad 108。然后从第 112 字节起是 8 个 ProbeCascadeData，每个 48 字节：position[3] 0-11，to_probe 12-15，probe_world_offset[3] 16-27，to_cell 28-31，pad[3] 32-43，exposure_normalization 44-47。
mapping-SdfgiData-lab-layout = 实验室 CPU 模型（godot/sdfgi）按相同位布局打包。
mapping-SdfgiHistory-engine-name = 探针球谐历史纹素（lightprobe_history_tex）及其累计和（lightprobe_average_tex）
mapping-SdfgiHistory-exact = R16G16B16A16_SINT，每层 289 x 272 个 texel，每个 cascade 有 history_size 层（默认 30）：列为 probe 的 x + 17 * z，行为 y * 16 + 系数序号（0-15），layer 为该次更新的槽；rgb = 1024 * 4 / ray_count * 该系数的光量之和，a = 1024 * 4 / ray_count（8 条射线时为 512），均限制在 -32768..32767。累计和为 R32G32B32A32_SINT，289 x 272，是各 layer 的精确和。
mapping-SdfgiHistory-lab-layout = 实验室 CPU 模型（godot/sdfgi）按相同位布局打包。
mapping-SdfgiLightprobe-engine-name = 八面体探针纹素（lightprobe_data，R32_UINT，按 E5B9G9R9_UFLOAT 读取）
mapping-SdfgiLightprobe-exact = 每个 texel 一个 uint，按 E5B9G9R9 读取：红尾数第 0-8 位，绿 9-17，蓝 18-26，共享 5 位指数第 27-31 位（编码前通道限制在 [0, 65408]）。数组为 2312 x 136，每个 cascade 2 个 layer：layer c 为 irradiance，layer cascades + c 为 radiance；probe p 占据位于 ((p.x + 17 * p.z) * 8, p.y * 8) 的 8 x 8 块，6 x 6 的贴图从 +1 开始，外围有一个 texel 的边框。
mapping-SdfgiLightprobe-lab-layout = 实验室 CPU 模型（godot/sdfgi）按相同位布局打包。
mapping-SdfgiOcclusion-engine-name = 遮蔽数据 word（occlusion_data，R16_UINT，按 R4G4B4A4 读取）
mapping-SdfgiOcclusion-exact = 32 位 word 中的八个 4 位值（occ * 15，截断）：类别 0 在第 12-15 位，1 在 8-11，2 在 4-7，3 在 0-3，4 在 28-31，5 在 24-27，6 在 20-23，7 在 16-19。低 16 位是 256 x 128 x (128 * cascade 数) 的 R16_UINT 贴图中 texel (x, y, z + cascade * 128)，高 16 位是 texel (x + 128, y, z + cascade * 128)；按 R4G4B4A4 读取时，R 为第 12-15 位，G 为 8-11，B 为 4-7，A 为 0-3。
mapping-SdfgiOcclusion-lab-layout = 实验室 CPU 模型（godot/sdfgi）按相同位布局打包。
