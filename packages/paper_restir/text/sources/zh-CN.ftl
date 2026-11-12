# paper_restir: the citation index's text in Chinese (Project Fluent; lab/src/sources.rs). The English is the index's
# own, written by src/paper/restir/source_index.py; a key names what it translates: `entry-<key>-inspect`,
# `chapter-<n>-title`, and `mapping-<lab>-engine-name`, `-exact`, `-lab-layout`.

entry-problem-inspect = 问题：用蒙特卡洛积分计算数百万动态光源的直接光照，即使离线渲染也很难。
entry-approach-inspect = 方法：重采样重要性采样，配合一个只存放已接受样本的固定大小的 reservoir。
entry-reservoir-intro-inspect = 不用比固定大小数组更复杂的数据结构；每个像素复用邻居的统计信息来改进自己的采样 PDF。
entry-pdfs-not-colors-inspect = 降噪器复用的是颜色，而这里复用的是采样概率，因此可以做到无偏。
entry-biased-mode-inspect = 有偏模式进一步降低噪声，代价是几何不连续处略微变暗。
entry-first-vertex-inspect = 论文只考虑相机路径第一个顶点处的直接光照。
entry-eq1-inspect = y 处的直接光照：在发光表面上对 BSDF、发射辐亮度、几何项和可见性的乘积积分。
entry-eq3-inspect = 重要性采样：从源 PDF p 取 N 个样本，对 f/p 取平均；只要 f 不为零处 p 都大于零，就是无偏的。
entry-eq4-inspect = MIS：从多种策略取样并加权；权重之和为一时无偏；平衡启发式。
entry-eq5-inspect = RIS 从源 p 抽取 M 个候选，按权重 w = p̂ / p 成比例地选出一个。
entry-eq6-inspect = 单样本 RIS 估计器：f(y) / p̂(y) 乘以候选权重的平均值，用来修正 y 只是近似服从 p̂。
entry-ris-unbiased-inspect = 当 M、N ≥ 1，且 f 不为零处 p 和 p̂ 都为正时，RIS 无偏；此后论文取 N = 1。
entry-ris-converges-inspect = 随着 M 增大，选出的样本的分布趋近目标 p̂。
entry-alg1-inspect = 算法 1：生成 M 个候选及其权重，建立归一化的 CDF，再从中抽取一个下标。
entry-wrs-inspect = 加权 reservoir 采样：对数据流只扫描一遍，每个元素被选中的概率是它的权重除以总和。
entry-eq9-inspect = 更新时，新样本以其权重除以累计和的概率替换保留的样本；之前的每个样本都按权重比例留下。
entry-alg2-inspect = 算法 2：reservoir 保存输出样本、权重之和和计数 M。
entry-alg3-inspect = 算法 3：把 RIS 改为流过一个 reservoir，由权重和得到 W（第 8 行），着色为 f(y) · W（第 11 行）。
entry-streaming-cost-inspect = 流式处理让存储变为常数，但计算量仍与 M 成线性。
entry-combine-inspect = 合并 reservoir：把每个 reservoir 的样本当作新样本送入一个新的 reservoir，等价于对它们合并后的数据流重采样。
entry-reweight-inspect = 邻居的样本按本像素的目标重新加权，乘以 p̂_q / p̂_q′，写作 p̂_q(r.y) · r.W · r.M。
entry-alg4-inspect = 算法 4：用每个 reservoir 的样本更新，权重为 p̂_q · W · M；M 是它们的 M 之和；W 由权重和得到。
entry-spatial-inspect = 空间复用可以重复，每一轮读取上一轮的结果：O(nk + M) 的计算量，换来 k^n M 个有效候选。
entry-temporal-inspect = 时间复用：保存每个像素的最终 reservoir，在下一帧合并，于是每一帧复用了之前所有的帧。
entry-visibility-inspect = 可见性复用：复用之前先测试每个像素选中的样本，被遮挡的就丢弃。
entry-alg5-inspect = 算法 5：候选、可见性、时间复用、n 轮空间复用、着色；最后返回 reservoir 给下一帧。
entry-bias-source-inspect = 复用相邻像素的候选会混合来自不同 PDF 的样本，而标准 RIS 并不处理这种情况。
entry-eq12-inspect = W 是一个随机权重，代替 1/p(y)；要无偏，它的期望必须等于 1/p(y)。
entry-weight-meaning-inspect = p̂_q(r.y) · r.W 就是 RIS 权重 p̂ / p，其中 r.W 估计了 PDF 的倒数，再乘以它背后的 r.M 个候选。
entry-eq19-inspect = 当某些候选 PDF 为零时，W 的期望是 1/p(y) 的 |Z(y)|/M 倍：偏小，所以图像偏暗。
entry-fig7-inspect = 图 7：用常数 PDF 和阶跃 PDF 采样线性目标 2 − 2x；比较 RIS 的权重与真实的 PDF 倒数。
entry-eq20-inspect = 无偏 RIS：把 1/M 换成权重 m，它在可能产生该样本的候选上求和为一。
entry-naive-m-inspect = 朴素权重 1/|Z|：无偏，但当某个 PDF 接近零而不为零时噪声很大。
entry-eq22-inspect = 取 m 为候选 PDF 的平衡启发式，同时解决了偏差和噪声。
entry-talbot-cost-inspect = Talbot 等人的另一种权重需要 O(M²) 次 PDF 计算；论文的做法只要 O(M)。
entry-alg6-inspect = 算法 6：Z 累加那些在该样本处 p̂ 不为零的来源的 M；W 使用 m = 1/Z（第 6-11 行）。
entry-approx-pdf-inspect = 每个来源像素的 p̂ 代替其样本未知的 PDF，在那个 PDF 为零处它也为零；启用可见性复用时，可见性也是它的一部分。
entry-unbiased-cost-inspect = 启用可见性复用时，无偏合并要为每个邻居多发一条阴影光线。
entry-candidates-param-inspect = M = 32 个候选：按功率选一个发光三角形，再在其上均匀取一点（p ∝ Le）；有环境贴图时 25% 取自环境贴图。
entry-alias-inspect = 用别名表（Walker 1974）选取光源。
entry-target-param-inspect = 目标 p̂ ∝ ρ · Le · G，不含阴影；材质为 Lambert 底层上的一层 GGX 电介质。
entry-neighbors-param-inspect = k = 5 个邻居（无偏时为 3），在 30 像素半径内由低差异序列随机取点；固定的邻居会产生瑕疵。
entry-temporal-pick-inspect = 时间邻居：用运动向量把当前像素的位置投影到上一帧。
entry-geometry-test-inspect = 有偏算法会拒绝相机距离相差超过 10%、或法线夹角超过 25° 的邻居。
entry-spatial-passes-inspect = n = 2 轮空间复用（无偏时为 1）。
entry-sample-count-inspect = 无偏算法 N = 1；有偏算法每个像素保存四个 reservoir。
entry-storage-inspect = 每个像素只保存它的 reservoir：选中的样本 y、M 和 W。
entry-clamp-inspect = 上一帧的 M 被截断为至多本帧的 20 倍，限制过时的时间样本。
entry-results-setup-inspect = 误差相对于高样本数的无偏参考图，用相对平均绝对误差表示。
entry-biased-vs-unbiased-inspect = 有偏方法方差更小，代价是阴影边界、锐利高光和复杂几何处的能量损失。
entry-many-lights-inspect = 多光源采样：Ward 和 Shirley 等人把光源分为重要与不重要；光源层次结构以亚线性时间采样它们。
entry-hierarchies-inspect = 与光源层次结构相比：无需维护数据结构，且样本考虑了 BSDF 和可见性。
entry-tiled-inspect = 分块着色限制每个分块的光源数，从而让每个光源的影响范围有偏。
entry-path-reuse-inspect = 路径复用每复用一次就要追踪一条可见性光线；ReSTIR 只为少数样本追踪光线。
entry-ratio-inspect = 比率估计器、加权均匀采样和加权重要性采样：有偏但一致；RIS 可以看作让它们变为无偏。
entry-reservoir-history-inspect = reservoir 采样始于 20 世纪 80 年代初，用来在磁带上选取数据而无需倒带。
entry-denoisers-inspect = 降噪器与之正交：在 ReSTIR 尚未收敛时可以改进它的输出。
entry-conclusion-inspect = 结论：降噪被移入渲染器内部，过滤的是 PDF 而不是颜色。
entry-limitations-inspect = 局限：像素间缺乏相关性时质量下降：遮挡变化处、光照不连续处、复杂几何和快速移动的光源。
entry-screen-space-inspect = 图像缓冲把方法限制在主命中点；世界空间的复用是未来的工作。

chapter-1-title = 论文及其问题
chapter-2-title = 相关工作
chapter-3-title = 流式 RIS
chapter-4-title = 空间复用
chapter-5-title = 时间复用与可见性复用
chapter-6-title = 偏差，以及如何消除
chapter-7-title = 在这台机器上的实验
chapter-8-title = 结论

mapping-Reservoir-engine-name = 像素的 reservoir：选中的样本 y、它代表的候选数 M，以及概率权重 W（§5，“Reservoir storage”）
mapping-Reservoir-exact = 每个像素保存 y、M 和 W；N 个样本时每个像素 N 个 reservoir。
mapping-Reservoir-lab-layout = 每个像素、每个阶段一个 Rgba32Uint 纹素：光源下标、以两个 16 位坐标表示的光源上的点，再是 W 和 M 的浮点位。
