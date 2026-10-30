title: O3DE 如何照亮一帧
subtitle: O3DE 的不透明帧，一个 pass 接一个 pass：移植到 WGSL 并对照 O3DE 源码
locale: zh-CN
glossary: ["StandardPBR", "forward pass", "target", "GGX", "Fresnel", "microfacet", "albedo", "F0", "roughness", "metallic", "BRDF", "cubemap", "mip", "split sum", "readout", "WGSL", "decal", "tile", "bin", "stencil", "probe", "SSAO", "clear coat", "lighting channel"]

# 1 | O3DE 的不透明帧
summary: Forward 像素着色器、它的 target，以及其后的 pass。

## c1-room | 一盏灯，一种材质

这是 Open 3D Engine 的 Atom 渲染器，用它的 {@ codespan="StandardPBR"} 材质着色器照亮实验程序的书房，着色器已移植到 WGSL。

房间里只有一盏灯：一盏 simple point light，O3DE 最朴素的点光源，没有形状，也没有阴影。

你将看到的每个公式都来自 O3DE 2.8 版，直接读自它的源码。

这里的每个表面都被照亮两次。

一次来自点光源，在 forward pass 中；一次来自天空，在它之后运行的 pass 中。

## c1-frame | 一个像素在一帧中的路径

这是一个像素在一帧中经过的路径。

深度预 pass 先画出场景的深度，光源剔除再把光源分到屏幕 tile 中。

forward pass 先计算材质，再应用 decal，然后处理每个光源，把漫反射和高光写入两个独立的 target。

它还写出乘以 diffuse response 的 albedo、带 roughness 的高光颜色，以及法线，供后面的 pass 读回。

然后全屏 pass 加上天空的漫反射和反射（来自天空与反射 probe）；SSAO 让漫反射变暗，合并 pass 再把两者相加。

在用 O3DE 默认模板创建的项目中，{@ codespan="DiffuseProbeGrid"} gem 会用它自己的合成 pass 替换漫反射 pass；它施加同样的天空光，按 probe grid 留下的权重加权，并且只在基于图像的光照标志开启处施加。

## c1-code | Forward 像素着色器

用伪代码写出来，forward 像素着色器很短。

它根据材质的输入计算表面，并初始化光照数据。

然后它加上像素所在屏幕 tile 列出的光源：先是方向光，再是 simple point 与 spot light，然后是面光源。这个房间只有一盏 simple point light。

在这里，基于图像的光照只对透明材质运行，或者在 mesh 或材质要求时只做高光部分；不透明材质由全屏 pass 获得它。

漫反射和高光写入不同的 target，由合并 pass 相加。

## c1-views | 把像素拆开

我们把结果拆开来看。

这是点光源单独的漫反射光照。

这是它的高光光照：小而锐利的亮点，在灯下方的地板上最亮。

这是全部加在一起的结果，包括 forward pass 之后的 pass 加上的天空光。

## c1-sources | 源码在哪里

这些是本章跟随的函数。

本视频的每个结论都引用 O3DE 源码中的一个文件、一个符号和一行，构建过程会检查每一行是否仍在那里。

# 2 | 光源剔除
summary: tile、深度 bin，以及 forward pass 遍历的列表。

## c2-why | 一个 tile 需要哪些光源

以上是整帧的概览；下一部分看它的第一个光照步骤：light culling。

在 forward pass 之前，O3DE 先决定屏幕的每个部分需要哪些光源。

这里，二十四盏小的 simple point light 悬在房间上方，每盏都用一个点标出。

屏幕被切成十六乘十六像素的 tile，每个 tile 有自己的光源列表。

伪彩色中，每个像素显示它所在 tile 在它的深度上保留了多少光源：深蓝表示没有，黄色表示八盏或更多。

三个 compute pass 构建这些列表，在深度预 pass 之后、任何着色之前。

## c2-prepare | 每个 tile 的深度

第一个是 {@ codespan="LightCullingTilePrepare"}，每个像素一个线程，每个 tile 一个线程组。

它找出 tile 最近的深度，以及除天空之外最远的深度，不透明与透明物体都算在内，所以以天空为背景的墙仍然很浅。

它在两者之间标记一个三十二段的掩码：每个不透明 sample 所在的那一段各占一位，透明表面则填满其最近与最远深度之间的所有段。

并按深度跨度（以米计）以二为底的对数，把范围分成一、二、四或八个深度 bin。

这里画出的盒子是被检查点所在的 tile，在视图空间中，深度 bin 画成切片。

## c2-cull | 把光源与 tile 测试

第二个 pass 是 {@ codespan="LightCulling"}，把每盏灯与每个 tile 测试。

当光源的球与 tile 的盒相交，并且它的深度范围碰到掩码中有表面的段时，它被保留。

每盏保留的光源写成它的索引左移十六位，再加上它所到达的每个深度 bin 的一位。

光源按类型分组：先是 decal，然后是 simple point light、spot light 和面光源，每组以结束标记收尾。

O3DE 的线程按完成的先后追加一组中的光源，decal 除外，因为它们会被排序；实验室按索引顺序追加光源，所以它的画面不会变化。

光源半径变大时，更多 tile 会保留它们。

## c2-forward | 每个深度 bin 一个列表

第三个 pass 是 {@ codespan="LightCullingRemap"}，把每个 tile 的列表改写成每个深度 bin 一个列表。

forward pass 找到自身深度所在的 bin，只遍历那一个列表。

在被检查的点上，readout 统计 tile 列表中的光源、它的 bin 保留的光源，以及球真正照到该点的光源。

带圈的点是该点的 bin 保留的光源，每盏都用虚线连到该点。

现在让光源移动：每盏光穿过 tile 和深度 bin 时，颜色和该点的列表都随之改变。

实验室把 GPU 上每个 tile 的列表与 CPU 副本比较，并检查照到表面的光源都没有从它的 bin 中漏掉。

## c2-finding | 隐含的顺序，读自代码

一个发现：forward pass 还会遍历一组 polygon light。

但剔除 pass 从不写出这一组，所以在默认开启光源剔除时，polygon light 从不照亮任何东西。

第二个发现来自同一个顺序：只有在启用点光源（punctual light）时，forward pass 才遍历两组 simple light。

在某个材质上关闭这个选项，之后的面光源就会读到排在它们前面的光源的组；相比之下，decal 仍会遍历自己的组。

## c2-sources | 源码在哪里

这些条目对应三个剔除 pass、tile 迭代器，以及关于其分组顺序的两个发现。

# 3 | 表面
summary: Metallic 工作流与 roughness 的重映射。

## c3-inputs | 材质提供了什么

光源已经分进了列表；下一部分转向它们照亮的对象：表面。

每个光照函数都从同一条表面记录开始。

材质提供 base colour、metallic 值、specular F0 factor 和 roughness。

O3DE 由这些值推导出光照真正使用的量。

## c3-metal | Metallic 工作流

Albedo 等于 base colour 乘以一减 metallic，所以金属根本没有漫反射颜色。

随着 metallic 增大，specular F0 从电介质的值过渡到 base colour。

电介质的值是 0.08 乘以 factor，所以默认 factor 0.5 得到 0.04，这是大多数油漆和塑料的反射率。

介于零和一之间的 metallic 值对两者做线性混合。

## c3-rough | Roughness 平方两次

Roughness 按感知上均匀的尺度来设置。

O3DE 把它截断到至少零点零二四，平方得到 alpha，再平方一次。

所以 roughness 为一半时，alpha 平方是十六分之一，之后 anti-aliasing 还会加宽它。

看着球体，roughness 从抛光扫到哑光。

## c3-aa | Specular anti-aliasing

然后，默认开启的 specular anti-aliasing 在法线逐像素快速变化处加宽 alpha 平方。

它加上一个核：法线跨一个像素横向与纵向变化平方和的一半，最多零点一八。

然后据此重新计算 alpha 与线性 roughness，所以后面的每一项都看到更宽的 lobe。

在光滑的球上，加宽出现在靠近边缘处，那里法线跨过一个像素转得最快。

实验室像 O3DE 一样，用法线的精细屏幕空间导数求这些导数。

## c3-occlusion | 遮蔽贴图

材质还可以带两张遮蔽贴图：漫反射环境光遮蔽，以及高光遮蔽，有时叫作 cavity 贴图。

每张取贴图的红色通道，再取 factor 次幂，factor 默认为一。

没有贴图时两者都为一，什么也不变。

在 forward pass 的最后，漫反射光乘以环境光遮蔽，高光乘以高光遮蔽。

albedo target 把两者都带给 skylight 的 pass，高光遮蔽放在它的 alpha 中。

调高 factor 会加深凹处；把它降到接近零则让贴图淡出。

## c3-examples | 每个物体，同样的规则

同样的规则适用于房间里的每个物体。

把茶壶变成金属，它的绿色就离开漫反射光，进入高光。

把盒子变粗糙，它的高光就扩散开，几乎看不见。

它们在计算任何光照之前，都要经过同样的两个函数。

## c3-sources | 源码在哪里

表面来自这四个地方：材质的计算、metallic 工作流、它的常量，以及 roughness 的重映射。

# 4 | Decal
summary: 在光照之前混合进表面的盒投影纹理。

## c4-what | 盒子里的一张纹理

在任何光照到这个表面之前，decal 可以先改变它，这就是下一部分。

decal 把一张纹理投射到落在一个盒子里的任何表面上。

这里，一块颜料泼溅和一个面板网格铺在地面上；盒子和箭头标出它们的体积和投影轴。

decal 默认开启，而且直接修改表面本身，在设置光照数据之前。

所以每盏灯，以及 forward pass 之后的 skylight pass，看到的都是 decal 改过的 albedo、F0 和法线。

## c4-apply | ApplyDecal

{@ codespan="ApplyDecal"} 通过 decal 的旋转和半尺寸把点移进它的盒子，在负一到一之外就跳过。

盒子的 x 与 y 成为纹理坐标，v 被翻转。

base map 乘以 decal 的颜色，按贴图的 alpha、decal 的 opacity 与角度衰减之积替换 albedo。

F0 也会变，变化量为该值乘以 metallic，所以金属上的 decal 会改变它反射的颜色。

它的 normal map 弯折法线，这里是面板之间的凹槽。

normal map 的两个通道被换回来，z 被重建，和 O3DE 的任何 normal map 一样。

## c4-angle | 随角度淡出

每个 decal 随表面与投影轴之间的夹角而淡出。

余弦被截断在零点五五，再 smoothstep 到一，所以 decal 在大约五十七度处消失。

把泼溅的轴从地面法线倾斜开，它就淡出了。

attenuation angle 设置默认为一，在不淡出与完全淡出之间插值。

## c4-order | 哪个 decal 在上面

decal 重叠时，它们的顺序决定哪个在上面。

光源剔除把 decal 放进每个 tile 列表的第一组，用包住盒子的球来表示。

然后按 sort key 排序，key 相同时按索引排序，所以 key 更大的后应用，盖在上面。

网格保持默认 key 十六，泼溅是十七，所以泼溅压在网格上面。

把泼溅改成十五，网格就盖住了它。

forward pass 在设置任何光照之前，先遍历这个排好序的组。

## c4-colour | 两种色彩空间

O3DE 在 ACEScg 中计算光照：每个材质颜色和光源颜色在输入时都从 linear sRGB 转换过来。

一个发现：decal 的颜色按给定值存储，不经转换就混合进 ACEScg 的 albedo。

readout 显示 b，也就是最上层 decal 混合进来的颜色（贴图乘以它的颜色），以及同样数值作为材质颜色时会变成什么。

接近灰色的颜色两者几乎相同；饱和的 decal 色相会偏移。

## c4-sources | 源码在哪里

这些条目展示 decal 在 forward pass 中的位置、每个 decal 如何混合，以及它们如何被剔除和排序。

# 5 | Simple point light
summary: 平方反比、半径窗口、没有阴影、自动半径。

## c5-record | 一个位置、一个强度、一个半径

表面已经准备好，decal 也已应用；接下来是照到它的光，先从最朴素的一种开始：simple point light。

simple point light 保存一个位置、以 candela 计的 RGB 强度、衰减半径平方的倒数，以及它的 lighting channel 和 GI 标志。

它的强度随距离按平方反比衰减：地板上的虚线。

然后一个窗口函数让它在衰减半径处平滑地降到零：实线。

窗口函数为 {@ formula="w = \big(1 - (\lit{1}{d^2} / r^2)^2\big)^2" display=yes colors="#ffd98a"}：1 减去（距离平方与半径平方之比）的平方，再整体平方一次。

在半径的一半处它仍保留百分之八十八，所以实线紧跟虚线，只在边缘附近下降。

Readout 跟踪球体上的一个点：它的距离、falloff 比值、窗口值，以及到达那里的照度。

六 candela 在不到两米处给出大约一点七 lux，这是在窗口函数之前。

## c5-radius | 衰减半径

在半径之外，这盏灯被完全跳过，所以半径也限制了它的开销。

增大半径，窗口就伸得更远。

在默认的自动模式下，半径是光源 lumen 按其颜色亮度加权、再除以零点一之后的平方根。

## c5-channels | Lighting channel

光源和网格还带有 lighting channel：各有五位。

只有当二者的掩码有共同位时，光源才照亮表面。

组件一开始五个 channel 全部开启，所以默认情况下每盏灯都照到每个网格。

给光源 mask 一、给球 mask 二，球就不再接收它：这里只显示点光源的漫反射。

skylight、反射 pass 和 decal 都忽略 channel。

一个发现：在方向光循环中，属于其他 channel 的光源会结束整个循环，所以它后面的方向光也被跳过。

## c5-shadows | 没有阴影

Simple point light 不投射阴影。

光到达每一个朝向它的表面，即使在盒子后面也一样。

没有阴影贴图，又按屏幕 tile 剔除，每盏灯的开销只比给半径内的像素着色多一点。

## c5-sources | 源码在哪里

光源的记录、它的衰减、跳过它的循环，以及自动半径都列在这里。

# 6 | GGX 高光
summary: N、L、V 与 H；D、G、F 三项；高光波瓣。

## c6-vectors | 四个方向

这样就得到了该点处的光强；表面把其中多少送回来由 BRDF 决定，先讲它的高光部分：GGX。

高光光照取决于一个点上的四个方向。

表面法线 N。

指向光源的方向 L。

指向观察者的方向 V；我们从蓝色的观察者看过去，让四个方向都位于正对我们的平面内。

还有半程向量 H，位于 L 和 V 的正中间。

法线沿 H 的 microfacet 会把光直接反射到观察者眼中。

## c6-terms | D、G 与 F

{@ codespan="SpecularGGX"} 把三项 D、G、F 相乘，再乘以 N dot L 和多次散射补偿。

D 是 GGX 法线分布，即朝向 H 的微表面法线的密度；对于光滑表面，它远大于一。

在伪彩色中，D 只在 H 与法线重合处明亮：那是高光的核心。

G 是高度相关的 Smith 项：这些 microfacet 中既没有被遮挡、也没有被挡住视线的比例。

O3DE 返回的值已经除以 4、N dot L 和 N dot V，省去了最后的除法。

F 是 Schlick 的 Fresnel，以 H dot V 计算：当光线与视线掠过微表面时，反射率趋向白色。

这个乘积就是高光响应；再乘以光源强度，就是高光光照。

## c6-sweep | 移动光源

现在看这个点上的数值和它的波瓣。

粉色曲线是这个平面内每个出射方向的高光响应，它绕着 L 的镜面方向倾斜。

把光源升向法线，波瓣也随之立起来。

把它降向地平线：N dot L 下降，Fresnel 增大，波瓣被拉长。

随着光源移动，readout 跟踪每一项。

## c6-rough | Roughness 让 D 变宽

Roughness 对 D 的影响比什么都大。

光滑的表面把光集中成又高又窄的波瓣，以及小而亮的高光。

粗糙的表面把同样的光摊薄。

在伪彩色中，D 从一个亮点扩大成一片宽广的光晕。

## c6-sources | 源码在哪里

{@ codespan="SpecularGGX"} 及其三项位于 O3DE 着色器库中三个简短的文件里。

# 7 | 漫反射与它的 response
summary: Lambert 漫反射乘以一减去考虑 roughness 的 Fresnel；对比 Disney 漫反射。

## c7-lambert | Lambert，加上一个 response

讲完 specular，下一部分转向每盏光的另一半：diffuse 及其 response。

{@ codespan="StandardPBR"} 的漫反射是 Lambert 的：albedo 乘以 N dot L，再除以 pi。

但它要乘以一个 diffuse response：一减去视角处的 Fresnel 项。

这个 Fresnel 考虑了 roughness：它在掠射角的值是一减 roughness，但不小于 F0。

所以从掠射角看光滑表面时，它留给漫反射的光更少，因为更多的光进入了高光。

没有这个 response，漫反射和高光加起来可能反射出比入射更多的光。

## c7-response | Response 在哪里下降

在伪彩色中，diffuse response 保持在接近 1，只在球体边缘附近下降。

绿色波瓣显示了每个视线方向的结果：接近一个半圆，在 response 下降的地方被压扁。

越光滑的表面在掠射角下降得越多。

## c7-disney | StandardPBR 不使用的漫反射模型

O3DE 的 BRDF 库还定义了归一化的 Disney 漫反射。

{@ codespan="StandardPBR"} 从不调用它；Skin 材质调用它，{@ codespan="EnhancedPBR"} 在开启次表面散射时也调用它。

它自带光进入和离开表面的 Fresnel 项，所以不使用 diffuse response。

回到 {@ codespan="StandardPBR"} 使用的 Lambert 模型。

## c7-sources | 源码在哪里

这些条目给出漫反射项、它的 response，以及哪些材质使用 Disney 模型。

# 8 | Clear coat
summary: 底层之上的电介质层：它的 F0、漫反射、高光与 IBL。

## c8-layer | 一层清漆

高光与漫反射合起来构成一层；clear coat 在其上再加一层透明的涂层。

Clear coat 在底层材质之上加一层薄而透明的涂层，就像油漆上的清漆。

它默认关闭；这里的球是粗糙的金属，外面有一层光滑的涂层。

涂层是 F0 为百分之四的电介质，有自己的 factor 和 roughness。

## c8-base | 下面的底层

在涂层下面，底层的 F0 会变，因为光现在是从涂层而不是从空气进入它。

O3DE 使用 Filament 的形式：一减五倍根号 F0，除以五减根号 F0，再平方，按 factor 混合进去。

diffuse response 按涂层在视角下的 Fresnel 缩小。

每盏灯的漫反射又按涂层在半程向量处的 Fresnel 再缩小一次。

一个发现：涂层让漫反射暗了两次，一次通过 response，一次针对每盏灯。

## c8-specular | 两个高光 lobe

每盏灯的高光变成底层 lobe 乘以一减涂层 Fresnel，再加上涂层自己的 lobe。

涂层的 lobe 是 GGX，配 Kelemen 的可见性项，alpha 为其 roughness 的平方，不低于零点零零零五。

所以球上显示出涂层的锐利高光，叠在金属宽阔的高光之上。

factor 增大时，锐利的高光变强，底下金属的颜色变暗。

## c8-ibl | 在 forward pass 中的天空反射

Clear coat 还把材质的天空反射移进 forward pass。

全屏反射 pass 对涂层一无所知，所以网格绘制时不带高光 IBL 的 stencil 值，该 pass 就跳过它。

在 forward pass 中，底层反射乘以一减涂层 Fresnel 的平方，再加上涂层自己的反射。

所以全屏的 probe pass 从不作用于有 clear coat 的网格；forward pass 会自己混合与网格重叠的最小 probe，按其 inner 与 outer 盒之间的权重。

## c8-sources | 源码在哪里

这些条目从涂层的输入一路跟到它的高光、漫反射和 forward pass 中的 IBL。

# 9 | BRDF 贴图与多次散射
summary: BRDFTextureCS、单次散射丢失的能量，以及补偿。

## c9-loss | 单次散射丢失的能量

涂层讲完了，底层自己的高光在高 roughness 时仍会丢失光能，而 O3DE 在启动时填充的一张纹理会把它补回来。

在粗糙表面上，一次 GGX 反弹会丢失能量。

照到一个 microfacet 的光可能先弹到另一个 microfacet 再离开，而单次散射公式直接把它丢掉了。

这是保留了这部分损失的粗糙金属球。

这是加上 O3DE 补偿后的样子：更亮，正如真实的粗糙金属那样。

## c9-texture | BRDF 贴图

为了知道损失了多少，O3DE 在启动时一次性填充一张二百五十六乘二百五十六的贴图，每个 texel 两个十六位浮点数；实验室用三十二位浮点数保存它。

每个 texel 代表一个视角和一个 roughness。

它取一千零二十四个 GGX 样本，把视线关于每个样本反射，并对地平线以上的样本按可见性乘以四、V dot H 和 N dot L，再除以 N dot H 加权。

它把总和拆成作用于 F0 的 scale A 和 bias B。

各行上下颠倒地存储，所以光照按 smoothness 而不是 roughness 来查这张贴图。

## c9-compensation | 把能量补回来

A 加 B 是单次散射保留下来的能量。

在 roughness 零点八和这个视角下，它只有大约百分之五十七。

补偿按 F0 的比例把缺失的部分加回来，采用的是 Turquin 的方法。

它的 shader option 默认开启，但 {@ codespan="StandardPBR"} 的材质属性默认关闭，所以材质必须自己开启它，O3DE 的金属预设正是这样做的；实验室把它打开了。

在伪彩色中，补偿随 roughness 增大。

## c9-finding | 贴图着色器的一个细节

移植时，贴图着色器的一个细节引起了注意。

它的采样器计算一减 cos theta 平方的平方根，没有做保护。

在本实验使用的 GPU 上，对于几乎镜面的波瓣，cos theta 可能被舍入到大于 1；这时平方根得到 {@ codespan="NaN"}，这个样本就丢失了。

所以 WGSL 移植在 roughness 低于大约 0.03 时会丢失能量。

实验程序因此在 CPU 上生成这张贴图；O3DE 自己的贴图是否有这种损失，取决于 GPU。

## c9-sources | 源码在哪里

贴图的着色器、它的尺寸、光照如何读取它，以及补偿，都在这些文件中。

# 10 | 透射（EnhancedPBR）
summary: 穿过厚物体与薄物体的光，以及它需要的 point light。

## c10-why | 来自背后的光

到目前为止，光只在表面反射；下一部分让光穿过物体：transmission。

薄的或半透明的东西，比如树叶、蜡或皮肤，会让光从背后透过来。

{@ codespan="StandardPBR"} 没有透射；{@ codespan="EnhancedPBR"} 加入了它，默认关闭，有厚物体和薄物体两种模式。

只要次表面散射和各向异性保持关闭，它的其他光照就与 {@ codespan="StandardPBR"} 相同。

这里茶壶是 {@ codespan="EnhancedPBR"}，从背后照亮；simple point light 已关闭。

## c10-light | 另一种点光源

simple point light 完全不产生背光。

所以茶壶背后的光源是 O3DE 的 point light，一种球形面光源，用编辑器默认的五厘米半径。

它的照明与 simple point light 相同，但把高光方向移到灯泡上离反射光线最近的点，并加上背光。

这里它不投射阴影。

## c10-thick | 厚物体

厚物体模式采用 Barré-Brisebois 与 Bouchard 的方法：光透过物体直射向观察者的程度。

把视线方向与被法线弯折的光源方向比较，取六次幂，再缩放。

然后按厚度衰减：e 的负四倍厚度次方，再乘以一减厚度。

厚度取阴影贴图厚度与材质厚度中较大的一个；这个茶壶用零点一五，默认值是一半。

所有光源的总和乘以 tint，在乘以遮蔽之后并入漫反射，所以遮蔽不会让它变暗。

没有透射时茶壶的背面是暗的；有了透射，光就透了出来。

## c10-thin | 薄物体，以及一个发现

薄物体模式采用 Jimenez 等人的方法：在阴影贴图测得的穿过物体的距离上计算扩散剖面。

它的注释说，没有阴影数据时就不产生背光。

一个发现：对 point light 来说，即使光源没有阴影，阴影分支也会执行，因为它的选项默认为 true。

这时阴影查找返回的厚度是零，而不是表示没有数据的负值。

所以在无阴影的 point light 后面，薄物体得到扩散核的最大值，即它能有的最亮的背光。

## c10-sources | 源码在哪里

这些条目涵盖透射的输入、球形光源、两种模式，以及阴影分支的发现。

# 11 | 太阳的级联
summary: 方向光、它的四个级联，以及不透明像素读取的那一个阴影值。

## c11-light | Lux，且没有大小

点光源只照到几米远；照到一切的是方向光，也就是太阳，它的阴影是下一部分。

它的强度是以 lux 计的照度：组件默认的四 EV100 就是四十 lux。

它还有一个角直径，默认半度；一个发现：forward pass 为它调整了一个方向，却把原始方向传给高光，所以在那里这个大小不起任何作用。

只有一个方向光投射阴影；其余的方向光不带阴影地照明。

实验室没有方向光，所以本章在 CPU 上按 O3DE 的算法，为一台站在房间里的观察相机画出这些计算。

## c11-splits | 视野的四段

阴影覆盖相机视野直到光源的阴影距离（默认一百米），若相机远平面更近则到远平面为止；这里设为十二米，让级联围在房间周围。

视野被切成四个级联，切分深度把对数划分与均匀划分混合起来。

整个视野只用一张 map，远处和近处就会分到同样多的 texel；默认比例零点九偏向对数划分，所以第一个级联又短又清晰，之后的一个比一个长。

比例为零时各段等长；为一时每段都比上一段长出一个固定倍数。

加大阴影距离，每一段都随之变长，于是每个 texel 覆盖房间中更大的范围。

## c11-fit | 每段一个球，并对齐到 texel

每一段用一个包围球拟合：球心位于到近角与远角距离相等的位置；若该位置超出远平面，则放在远平面上，房间里的前三段就是这样。

在光源坐标系中，级联的包围盒就是这个球的立方体，并朝光源方向拉长，以容纳它与视野之间以及相机背后的遮挡物。

然后它的角点向下取整到这张一千零二十四 texel 的 shadow map 的整数 texel。

相机转动时球的大小不变，所以每个盒子保持尺寸不变；对齐又让它只按整数 texel 移动，从而让阴影边缘不闪烁。

readout 给出每个级联的半径和它的 texel 大小：近处是毫米，远处是厘米。

## c11-select | 像素选择它的级联

为了找到自己的阴影，像素先沿法线移动二点五乘以二除以 map 尺寸，大约五毫米。

然后它取第一个能把它包含在距边缘至少一点五 texel 以内的级联：选择依据是 map 的范围，而不是像素的深度。

readout 显示球上被检查的点落在哪个级联，以及它在那里的 texel。

不透明像素在 forward pass 中从不采样这些 map：一个全屏 pass 把每个像素的可见度写进它自己的 target，加以模糊，forward pass 读取那一个值。

那个 target 是八位的，与输出同尺寸：每像素一个值，而不是每个 sample 一个。

## c11-others | forward pass 的其他光源

在方向光之后，forward pass 依次处理 simple light，然后是五种面光源。

Quad light 用 linearly transformed cosines 积分，查两张三十二乘三十二的表格，或者使用光源可以要求的更快的近似。

Disk、capsule 和 polygon light 各有自己的方法；实验室一个也没有移植，而且正如第二章所示，开启剔除时 polygon light 从不着色。

## c11-sources | 源码在哪里

先读 feature processor 中的划分、拟合与对齐，再读 shadow calculator 中像素的选择。

# 12 | 全局 skylight
summary: 离线的 GGX 与余弦预过滤；两个全屏 pass；没有遮蔽。

## c12-sky | 来自天空的光

forward pass 计算的光源就讲到这里；不透明帧在它之后加上的光来自天空。

第二个光源是天空。

O3DE 的全局 skylight 是基于图像的光照：一个环境，存为两个 cubemap。

这个镜面球直接反射它；实验程序的天空是一张照片，Poly Haven 的 Tree Lined Driveway，太阳被截断。

## c12-bake | 离线烘焙

这些 cubemap 由 O3DE 的图像处理器离线烘焙。

GGX 滤波把环境转换为高光 cubemap：每面二百五十六 texel，格式 R9G9B9E5，六级 mip，每级对应一档 roughness。

余弦滤波把它转换为每面三十二到一百二十八 texel 的漫反射 cubemap；实验室的是三十二。

另外，第九章的 BRDF 贴图提供 split sum 的各项。

## c12-prefilter | 预过滤高光 cubemap

高光 cubemap 的第 m 级 mip 按 m 除以 5 的 roughness 来滤波。

对每个 texel，滤波假设法线和视线都沿着这个 texel 的方向。

然后在它周围取 256 个 GGX 样本。

每个样本从比立体角匹配的 mip 再模糊一级的位置读取，并在两级 mip 之间混合，从而掩盖了样本这么少带来的噪点。

越粗糙的 mip 收集越宽的锥体，所以反射变模糊。

## c12-cosine | 漫反射 cubemap

漫反射 cubemap 是整个半球上的余弦加权平均，并按权重归一化。

所以它存的是辐照度除以 pi，漫反射 pass 直接乘以 albedo，不再做除法。

单独看，来自天空的漫反射光很柔和，被下方的地面和上方的蓝色染上颜色。

## c12-passes | 两个全屏 pass

运行时，forward pass 已经把乘以 diffuse response 和遮蔽的 albedo、F zero 以及 roughness 存下，每个都是八位。

漫反射 pass 用该 albedo 乘以法线方向上的漫反射 cubemap；在默认模板下，由 {@ codespan="DiffuseComposite"} 做同样的事。

反射 pass 沿反射向量、在 mip roughness 乘以 5 处读取高光 cubemap。

它应用 split sum，即 F zero 乘以 A 再加 B，带上补偿，并把 N dot V 限制在百分之一以上，然后乘以 skylight 的曝光和高光遮蔽。

漫反射 pass 直接加到 diffuse target 上；反射 pass 写入自己的 target，再由一个合成 pass 加到 specular target 上。

## c12-readout | 一个点上的各项

在选中的点上，readout 显示每一步：8 位的 roughness、它的 mip、预过滤的光，以及 split sum。

随着 roughness 增大，mip 随之增大，反射变暗并扩散。

## c12-occlusion | 天空没有阴影

全局 skylight 不投射阴影：只有遮蔽贴图和默认开启的屏幕空间环境光遮蔽会让它变暗。

实验室的房间没有遮蔽贴图，所以在房间里只有 SSAO 让天空的光变暗，在角落和物体下方。

对于室内，O3DE 用反射 probe（见下一章）取代天空的反射，用 diffuse probe grid 取代它的漫反射，实验室没有实现后者。

## c12-sources | 源码在哪里

烘焙、target 和两个 pass 都在这些文件中；滤波库 {@ codespan="CubeMapGen"} 属于 AMD，采用 BSD 许可。

# 13 | 反射 probe
summary: probe cubemap、它们的 stencil 与混合权重、视差，以及合成。

## c13-why | 房间的反射

天空的 cubemap 对整个场景一视同仁；下一部分的 reflection probe 让局部拥有自己的反射。

在房间里，反射出开阔的天空看起来是错的：墙应该出现在反射中。

反射 probe 解决这个问题：每个都是从场景中某一点烘焙的 cubemap，带一个 inner 盒和一个 outer 盒。

实验室放置了两个，并在 CPU 上自己烘焙，从每个 probe 的中心对房间着色。

它们的 cubemap 每面二百五十六 texel，与默认的 Medium 烘焙质量一致，并像天空的那样预过滤。

在抛光的金属地面上，probe 把房间自己的墙放进了反射里。

## c13-bake | probe 烘焙了什么

烘焙一个 probe，会从 probe 的位置把场景渲染六次，立方体的每个面一次，每次是一个九十度宽的正方形视图。

O3DE 一次绘制一个面，画进一个一千零二十四 texel 的十六位浮点 target，等待五帧后读回，再转向下一个面。

展开之后，probe 一的各个面显示：前方是后墙，左边是红墙，右边是青绿色的墙，上方是天空，下方是地面。

编辑器把六个面写进一个 DDS 文件，O3DE 的图像构建器再用 skylight 自己的高光预设把它变成 probe 的 cubemap。

在默认的 Medium 质量下，就是每面二百五十六 texel 的 R9G9B9E5，以及六级 mip，每一级都用 GGX 为更粗糙的表面滤波。

实验室省去了大尺寸采集：它在 CPU 上直接着色二百五十六 texel 的面。

probe 二是右后角一个较小的 probe，后墙和青绿色的墙都近在身边。

## c13-ball | 可视化球

在编辑器中，每个 probe 还会在自己的位置显示一个球，默认开启。

它的材质是 roughness 为零的白色金属，所以它镜面反射 probe cubemap 最清晰的那级 mip。

采集时会排除这个球，所以 probe 不会把自己的球烘焙进去。

实验室画了自己的镜面球，它沿反射光线直接读取 cubemap；mip 越高，反射越模糊。

## c13-stencil | Stencil 与混合权重

forward pass 之后运行五个 pass：一个 stencil pass、一个混合权重 pass、全局 pass，以及 outer 与 inner 两个 probe pass。

stencil pass 绘制每个 probe 稍微缩小的 inner 盒，给盒内每个使用反射 pass 的表面的 stencil 加一。

在所有 inner 盒之外的表面保持值三。

在那里，混合权重 pass 加上每个 probe 的权重：在 inner 盒处为一，到 outer 盒处降为零。

probe 重叠处，权重相加。

## c13-blend | 混合 probe

随后全局 pass 只在 inner 盒之外运行，保留 probe 剩下的部分：一减去它们的权重之和，而权重之和最多为一。

outer pass 加上每个 probe 的反射乘以其权重，总和超过一时再除以总和。

在 inner 盒内，inner pass 直接用 probe 的反射替换。

probe 按从大到小绘制，所以大 probe 里的小 probe 最后写入。

与全局 pass 不同，probe 不乘高光遮蔽。

缩小 inner 盒，probe 混合的带就变宽了。

## c13-parallax | 视差校正

cubemap 是从一个点捕获的，但表面遍布整个盒子。

所以默认情况下，反射光线被追踪到 outer 盒，再从 probe 中心朝交点方向读取 cubemap。

所以 probe 的 outer 盒应当贴合它所在的房间：probe 在盒子之外看到的东西，会被当作位于盒面上来绘制，墙就会出现在错误的位置。

没有这个校正，反射中的墙就会错位；有了它，墙又重新对齐。

## c13-composite | 合成，以及一个发现

然后 composite pass 在 stencil 标记了表面的地方，把 reflection target 加到高光上。

一个发现：这个 target 被清成零点四的灰色，而全局 pass 从不在 inner 盒内运行。

所以关闭基于图像的光照时，inner pass 会丢弃，inner 盒内的表面就会在高光上加上这层灰色。

实验室复现了这一点，如 readout 所示。

## c13-sources | 源码在哪里

这些条目按顺序跟随五个反射 pass、probe 的高光，以及合成。

# 14 | Resolve、环境光遮蔽与合并
summary: 自定义的高光 resolve、半分辨率的 SSAO，以及合并。

## c14-resolve | 带钳制的高光 resolve

probe 合成之后，每一项都放在两个 target 中，每像素若干个 sample；最后的 pass 对它们做 resolve，让漫反射变暗，再把两者相加。

漫反射 target 用普通平均做 resolve，而高光 target 走一个自定义的 resolve。

每个 sample 先被压低到不超过四个对角邻居所有 sample 中最亮亮度的一点五倍。

然后在对数域对这些 sample 求平均，也就是几何平均，这样一个很亮的 sample 不会主导一条边缘。

在伪彩色中，它与普通平均的相对差（零到百分之十）只在 sample 差别很大的地方亮起：轮廓边缘，以及最小的高光。

O3DE 的管线渲染两个 sample，实验室渲染四个，并以同样的方式 resolve。

## c14-why | 角落与缝隙

sample 完成 resolve 之后，环境光遮蔽让漫反射 target 变暗。

天空的光照进角落和缝隙，和照在开阔地面上一样亮。

屏幕空间环境光遮蔽只凭深度缓冲让这些地方变暗，主管线默认运行它。

它在反射与次表面散射之后运行，只乘 diffuse target，直接光和天空光都一样。

这是它的结果，没有遮挡的地方是白色。

## c14-compute | 五十二个采样

它从线性深度开始：每个像素第一个 MSAA 采样的视图空间深度。

然后在半分辨率下工作，保留每个二乘二的左上像素。

{@ codespan="SsaoCompute"} 由深度重建每个像素的位置和法线，并在其周围取五十二个采样：十二个近处的，以及与邻居共享的十轮、每轮四个。

每个采样贡献法线与指向它的方向之间的余弦，权重为一除以距离平方加零点零七，遮蔽就是它们的加权平均。

结果是一减遮蔽乘以 strength，再取二点二次幂，存为八位。

strength 默认为一，采样半径默认为半分辨率高度的百分之五。

半径越大，影响越远、变暗的范围越广；半径小则只留在最窄的缝隙里。

## c14-blur | 深度感知模糊

原始结果有噪点，所以用深度感知模糊先竖直、再水平地平滑它。

它逐像素推进，保留前一个值的三分之二，深度斜率变化处保留得更少，所以边缘保持锐利。

每个输出一半来自其前八个像素开始的链，一半来自其后八个像素开始的链。

这些是模糊 pass 自己的默认值，在场景没有环境光遮蔽组件时使用；组件的默认值是零点八五和二百。

没有模糊，采样的图案就会透出来；有了模糊，图案就消失了。

## c14-upsample | 回到全分辨率

{@ codespan="DepthUpsample"} 从最近的四个半分辨率像素把它恢复到全分辨率，按距离和深度的接近程度加权。

然后 diffuse target 乘以它，合并 pass 再加上未受影响的高光。

实验室把这些阶段在 GPU 上逐一与 CPU 副本核对，误差在一个八位级以内。

由于它只看到深度，SSAO 对屏幕外或最近表面之后的东西一无所知。

## c14-sources | 源码在哪里

这些条目先跟随高光 resolve，再跟随 SSAO 从线性深度经过采样、模糊与上采样，直到对漫反射的乘法。

# 15 | 回顾
summary: StandardPBR 做了什么，实验程序省略了什么。

## c15-recap | 不透明帧总结

这就是不透明帧的全部；下面按顺序再走一遍。

先是深度预 pass，然后光源剔除把光源和 decal 分进 tile 与深度 bin。

方向光的级联根据视野拟合，它的阴影被解析成每像素一个值。

forward pass 先把 decal 应用到表面，再处理每个光源：GGX 高光、按 response 缩放的 Lambertian 漫反射、启用时的 clear coat，以及 {@ codespan="EnhancedPBR"} 的透射。

全屏 pass 加上 skylight 和反射 probe，数据读回自 forward pass 的 target。

各 sample 完成 resolve（高光带邻域钳制），SSAO 让漫反射变暗，合并 pass 把两者相加。

实验室把这些公式移植到 WGSL，用 CPU 副本逐一核对，并引用它遵循的每一行代码。

它没有包含渲染出的阴影、面光源、默认关闭的屏幕空间反射，以及 diffuse probe grid。

本视频中的每个演示都可以在查看器里探索，滑块、伪代码和源码并排显示。

感谢观看。

