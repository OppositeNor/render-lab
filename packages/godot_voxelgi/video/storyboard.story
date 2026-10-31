title: How Godot's VoxelGI Lights a Room
subtitle: Baked voxel global illumination in Godot 4.7, followed through the source at the engine's sizes
engineVersion: 4.7.2-stable
commit: ed1daf0bf001b61586d9930840f2f1394092c079
date: 2026-10-31
fps: 30
width: 1920
height: 1080

# 1 | Where VoxelGI runs | 185 s
summary: What VoxelGI adds, the problem it solves, the parts it is made of, and what decides whether it runs.
sources: frame-order, voxelgi-gate, voxelgi-doc, mobile-stub, gles3-stub, node-defaults, quality-setting

## c1-room | 3d | VoxelGI adds the light that bounced off the room
{@ demo=room voxelgiMix=0 callout="" image="" panel=hidden cameraDrift=1 camera.yaw=24 camera.pitch=18 camera.distance=6.4}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[14, 16, 6.8], [26, 20, 5.2], [20, 17, 6.3]], "seconds": "beat"}}]}

{@m0> voxelgiMix=0..0.15}This is the lab's study room, lit by its ceiling panel and the sky alone: whatever the panel cannot see takes only the sky's flat light{<m0}.

{@gi> voxelgiMix=0.15..1}Now Godot's VoxelGI adds the light that bounced off the room{<gi}: {@vm> viewMode=0..1}the red wall warms the sphere and the floor beside it, and the corners fill in{<vm}.

{@vm2> viewMode=1..0}The light a surface receives from the room depends on everything it can see: too much to gather again, ray by ray, for every pixel and every frame{<vm2}.

{@off> voxelgiMix=1..0.3}VoxelGI's answer is to bake the room once into a grid of small boxes, called cells{<off}, {@on> voxelgiMix=0.3..1}light those cells, and let every pixel read them back through a few wide cones, each a cheap average of many cells{<on}.

## c1-map-bake | 2d | Baked once
diagram: frame
focus: BAKE, SDF

{@ callout=bake-entry} The bake runs once, usually in the editor: it turns the meshes in a box into an octree of cells, and adds a distance field, used later to find shadows.

## c1-map-light | 2d | Lit when something changes
diagram: frame
focus: LIGHT, TEX, DYN

{@ callout=relight-gate} At run time, light passes fill a three-dimensional texture from those cells, and run again only when a light or the data changes; moving objects are added to it every frame.

## c1-map-read | 2d | Read every frame
diagram: frame
focus: PRE, CONE, OPAQUE

{@ callout=process-gi} Every frame, a screen pass traces cones through that texture for the pixels it covers, and the opaque pass mixes the result into the image.

By the end you will be able to follow one pixel's light back through each of these parts.

## c1-parts | 3d | The parts, on the room
{@ demo=bounds callout="" image="" panel=hidden showBox=1 boundsStep=0 octreeLevel=-1 cellView=0 cutHeight=3.2 cameraDrift=1 camera.target=0,0.9,-0.3 camera.yaw=22 camera.pitch=30 camera.distance=8.5}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[18, 28, 8.0], [30, 34, 8.8], [24, 30, 8.2]], "seconds": "beat"}}]}

{@p1> octreeLevel=-1..7}{@p1c> cutHeight=3.2..1.4}Here are the baked cells of the room, one point each{<p1c}{<p1}.

{@p2> cellView=0..4}{@p2c> cutHeight=1.4..2.4}Each cell stores light{<p2c}{<p2}, {@p3> octreeLevel=7..5}and each coarser cell a blend of the eight inside it{<p3}.

{@ demo=pixel workedPixel=1 conesShown=0 coneSteps=16 specCone=0 panel=hidden}{@p4> conesShown=0..4}And a pixel reads that light through a few cones, which grow wider as they go{<p4}.

## c1-gate | 3d | Forward+ only, on whenever a VoxelGI is in view
{@ demo=room callout="" image="" panel=values focusRow=0 cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[16, 18], [8, 22], [14, 19]], "seconds": "beat"}}]}

{@ callout=voxelgi-doc}{@f1> focusRow=0..1}VoxelGI works only in the Forward+ renderer{<f1}: {@ callout=gles3-stub}the Compatibility renderer leaves its functions empty, {@ callout=mobile-stub}and the Mobile renderer pairs no geometry with it, so nothing reads its light.

{@ callout=voxelgi-gate}{@gate> voxelgiMix=1..0}There is no switch for it: its work runs in any frame whose culling finds a VoxelGI in view{<gate}.

{@ callout=node-defaults}{@sweep> focusRow=1..4}{@gate2> voxelgiMix=0..1}The node's defaults are a box of twenty metres a side, cut into one hundred twenty-eight cells along its longest axis{<gate2}{<sweep}.

{@ callout=quality-setting}{@f2> focusRow=4..12}{@q> quality=0..1}One project setting picks between four cones per pixel, the default, and six{<q}{<f2}; the other settings arrive in the chapters that use them.

## c1-box | 3d | The lab's box: six point four metres, five-centimetre cells
{@ demo=bounds callout="" image="" panel=values showBox=0 boundsStep=0 focusRow=-1 cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 30, 12], [38, 36, 10.5]], "seconds": "beat"}}]}

{@box> showBox=0..1}{@boxRow> focusRow=-1..0}The lab places one VoxelGI around its room, six point four by three point two by six point four metres{<boxRow}{<box}.

{@bRow> focusRow=0..4}{@lv> octreeLevel=3..7}At the default subdivision its finest cells are five centimetres wide{<lv}{<bRow}, {@bRow2> focusRow=4..6}{@cutb> cutHeight=3.2..1.0}about a million of them if the grid were full{<cutb}{<bRow2}.

{@lv3> octreeLevel=7..-1}{@bs> boundsStep=0..2}The lab bakes, lights, and reads this box with code ported from Godot's, and checks its ports against each other{<bs}{<lv3}.

## c1-recap | 3d | Next: the bake
{@ demo=room callout="" image="" panel=hidden voxelgiMix=1 cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[20, 18, 6.4], [32, 22, 6.0]], "seconds": "beat"}}]}

{@mix> voxelgiMix=1..0}So VoxelGI is a box of baked cells{<mix}, {@mix2> voxelgiMix=0..1}lit when the lights change, and read back by every pixel it covers{<mix2}.

{@vmr> viewMode=0..1}Everything it can light starts in that bake, which the next chapter follows{<vmr}.

## c1-sources | 2d | Where to read
sources: frame-order, voxelgi-gate, voxelgi-doc, mobile-stub, gles3-stub, node-defaults, quality-setting

These are the lines this chapter followed: the frame's VoxelGI steps, the gate, the renderers, and the defaults.

Every claim in this video cites a file, a symbol, and a line in the Godot checkout, and the build checks each line is still there.

# 2 | Baking the octree | 225 s
summary: The bake makes the box a cube of cells, sends every triangle down an octree, and samples it in each leaf it reaches.
sources: bake-entry, find-meshes, begin-bake, plot-mesh, plot-face-recurse, tri-box, leaf-scan, material-cache

## c2-map | 2d | The bake comes first
diagram: frame
focus: BAKE

Everything VoxelGI knows about the room comes from one bake, so that is where it starts.

## c2-collect | 3d | Static meshes inside the box are baked
{@ demo=bounds callout="" image="" panel=values showBox=1 boundsStep=0 focusRow=-1 octreeLevel=-1 cellView=0 cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[30, 32, 11], [44, 34, 10], [36, 30, 10.8]], "seconds": "beat"}}]}

{@ callout=bake-entry}{@sb> showBox=0.4..1}Pressing Bake starts the voxelizer on the node's box{<sb}.

{@ callout=find-meshes}{@cl> octreeLevel=-1..7}It walks the nodes beside the VoxelGI and collects every visible mesh whose GI mode is Static, the default, and whose bounds touch the box{<cl}.

{@ callout=plot-mesh}{@ct> cutHeight=3.2..1.2}Of those, only triangle surfaces are baked, and a triangle outside the box is skipped whole, not cut at its edge{<ct}.

{@ct2> cutHeight=1.2..3.2}The ceiling panel has no cells: the lab leaves it out, since its light comes from an area light instead{<ct2}.

## c2-cube | 3d | The box becomes a cube of cells
{@ demo=bounds callout="" image="" panel=values showBox=1 boundsStep=0 focusRow=0 cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[28, 30, 14], [40, 36, 16]], "seconds": "beat"}}]}

{@ callout=begin-bake}{@r1> focusRow=0..1}{@lc0> octreeLevel=-1..1}The longest axis gets two to the seventh, one hundred twenty-eight cells{<lc0}{<r1}.

{@ panel=code code=begin-bake highlight=shrink}{@cube> boundsStep=0..1}{@lc1> octreeLevel=1..2}A shorter axis halves its count while half the long side still covers it, so the box's three point two metre height gets sixty-four{<lc1}{<cube}.

{@ highlight=cube}{@cube2> boundsStep=1..2}Then every side is stretched to the longest, so the octree covers a cube with cells of one size{<cube2}: {@lc> octreeLevel=2..4}at the seventh level, five centimetres{<lc}.

{@ highlight=cell}{@lc2> octreeLevel=4..3}The top half of that cube has no cells at all: the height's sixty-four stop at three point two metres{<lc2}.

## c2-descent | 3d | A triangle goes down the octree
{@ demo=octree callout="" image="" panel=code code=plot-face highlight=octants workedTriangle=1 descentLevel=0 octreeLevel=-1 satStep=0 cameraDrift=0.5 camera.target=-0.01,0.24,-1.71 camera.distance=7}
{@ animate=[{"path": {"params": ["camera.distance", "camera.yaw"], "through": [[5.0, 50], [2.2, 58], [0.9, 60]], "seconds": "beat"}}]}

{@d0> descentLevel=0..0.5}Take one triangle of the teapot's spout{<d0}.

{@ callout=plot-face-recurse}{@d01> descentLevel=0.5..1}It enters at the root, the whole cube, which is cut into eight children{<d01}.

{@d1> descentLevel=1..3}{@ highlight=sat}Each child is tested against the triangle, and only the children it overlaps are made cells, {@ highlight=create}so the triangle carves a path one level deeper at a time{<d1}.

{@d2> descentLevel=3..7}{@ highlight=down}Seven levels down, the children are five centimetres wide, the leaves{<d2}, {@lvd> octreeLevel=-1..7}{@cutd> cutHeight=3.2..0.7}and the triangle has created only the handful of cells it touches{<cutd}{<lvd}.

## c2-sat | 3d | The test: a separating axis
{@ demo=octree callout="" image="" panel=values workedTriangle=1 descentLevel=5 octreeLevel=-1 satStep=0 focusRow=8 cameraDrift=0.3 camera.target=-0.15,0.15,-1.8 camera.distance=1.0}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[52, 22], [64, 28]], "seconds": "beat"}}]}

{@ callout=tri-box}{@sa> satStep=0..1}The overlap test looks for an axis that separates the box from the triangle: one along which their shadows do not overlap{<sa}.

{@d3> descentLevel=5..6}It tries nine axes made from the triangle's edges, then the box's own three, then the triangle's plane{<d3}; {@d35> descentLevel=6..5}the first that separates them rules the child out{<d35}.

{@d4> satStep=1..3}The readout names the axis that rejected the child drawn in red{<d4}.

## c2-all | 3d | Every triangle, sharing the cells
{@ demo=octree callout="" image="" panel=values workedTriangle=0 octreeLevel=3 cutHeight=3.2 focusRow=3 cameraDrift=1 camera.target=0,0.9,-0.3 camera.yaw=24 camera.pitch=28 camera.distance=7.4}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[18, 30, 7.2], [32, 26, 6.6], [26, 28, 7.0]], "seconds": "beat"}}]}

{@lv0> octreeLevel=0..3}{@lr0> focusRow=0..3}Every triangle of every mesh goes down the same way, and they share the cells they reach{<lr0}{<lv0}.

{@lv> octreeLevel=3..7}{@lvrow> focusRow=3..7}Level by level the cells shrink and multiply{<lvrow}{<lv}: the room ends with forty-two thousand two hundred eighty-nine leaves, of fifty-six thousand cells in all.

{@cut> cutHeight=3.2..0.7}The cells follow only surfaces{<cut}, {@cut2> cutHeight=0.7..2.0}so about four in a hundred of the million places in the grid hold a leaf: the octree is sparse{<cut2}.

## c2-leaf | 3d | A leaf samples its triangle on four by four columns
{@ demo=octree callout="" image="" panel=code code=leaf-scan highlight=axis workedTriangle=1 workedLeaf=1 leafColumns=0 octreeLevel=-1 cameraDrift=0.2 camera.target=-0.01,0.24,-1.71 camera.distance=0.32}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[50, 22], [62, 30], [56, 26]], "seconds": "beat"}}]}

{@ callout=leaf-scan}{@c0> leafColumns=0..1}At a leaf, the triangle's plane picks the axis it faces most{<c0}.

{@cols> leafColumns=1..16}{@ highlight=columns}The cell is cut into four by four columns along that axis, and each column the triangle crosses gives one sample{<cols}.

{@ highlight=accum panel=values}{@cr0> leafColumns=16..12}A sample adds the material's albedo, its emission, and the triangle's normal at that point{<cr0}.

{@ highlight=add}{@cr> leafColumns=12..8}The sums are scaled by a sixteenth, so a leaf the triangle half covers gets half its weight{<cr}: {@cr2> leafColumns=8..16}alpha is the share of columns hit, which later turns the sums into an average{<cr2}.

## c2-material | 3d | Materials become flat colours
{@ demo=octree callout="" image="" panel=hidden workedTriangle=0 octreeLevel=7 cellView=0 cutHeight=3.2 cameraDrift=1 camera.target=0,0.9,-0.3 camera.yaw=20 camera.pitch=26 camera.distance=6.8}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[14, 24, 6.4], [28, 30, 6.9]], "seconds": "beat"}}]}

{@ callout=material-cache}{@mv> cellView=1..0}A material is read once into two tables of one hundred twenty-eight texels a side: its albedo in linear colour, and its emission times its energy and the bake's exposure{<mv}.

{@cutm> cutHeight=3.2..1.0}Here each leaf shows the albedo it received{<cutm}; {@cutm2> cutHeight=1.0..2.2}the room's materials are flat colours, so every sample of a mesh is the same{<cutm2}.

## c2-recap | 3d | Next: finishing and packing the cells
{@ demo=octree callout="" image="" panel=hidden workedTriangle=0 octreeLevel=7 cellView=1 cutHeight=1.0 cameraDrift=1 camera.target=0,0.9,-0.3 camera.yaw=20 camera.pitch=26 camera.distance=6.8}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.distance"], "through": [[26, 6.4], [16, 7.0]], "seconds": "beat"}}]}

{@rc> cutHeight=1.0..3.2}So the bake keeps only the cells that surfaces pass through, and each leaf sums what its triangles left in it, shown here as their coverage{<rc}.

{@rc2> octreeLevel=7..5}Those sums are not yet what the GPU reads; the next chapter finishes the cells and packs them into bytes{<rc2}.

## c2-sources | 2d | Where to read
sources: bake-entry, find-meshes, begin-bake, plot-mesh, plot-face-recurse, tri-box, leaf-scan, material-cache

These are the voxelizer's functions, in the order the bake calls them.

# 3 | What the bake stores | 210 s
summary: Leaves are averaged, parents emptied, the cells sorted and packed into 48 bytes, and a distance field made; then everything is uploaded.
sources: fixup-leaf, fixup-parent, sort-cells, export-octree, export-data, rgbe9995, sdf-edt, sdf-encode, allocate-data, instance-texture

## c3-map | 2d | From cells to bytes
diagram: frame
focus: BAKE, SDF

The bake has left a tree of cells holding raw sums; before the GPU sees them, they are finished, sorted, and packed.

## c3-fixup | 3d | A leaf becomes an average
{@ demo=cells callout="" image="" panel=values fixupStep=0 focusRow=-1 octreeLevel=7 cutHeight=0.3 cameraDrift=0.2 camera.target=-0.01,0.24,-1.71 camera.distance=0.7}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[50, 20, 0.6], [60, 26, 0.8]], "seconds": "beat"}}]}

{@ callout=fixup-leaf}{@fr> focusRow=1..3}This teapot leaf holds the sums of every triangle that reached it, its alpha the total of their shares{<fr}.

{@fix> fixupStep=0..1}Finishing divides every sum by that alpha, so the leaf holds the average albedo and normal of its samples{<fix}, {@fr2> focusRow=3..9}and its alpha becomes one{<fr2}.

{@ panel=code code=fixup highlight=fight}{@nv> cellView=0..2}{@nc> cutHeight=0.3..0.5}If the samples' normals cancel out, shorter than one hundredth, the cell keeps no normal at all: then the light passes treat it as facing every way{<nc}{<nv}.

## c3-parents | 3d | Parents keep only their coverage
{@ demo=cells callout="" image="" panel=code code=fixup highlight=parent octreeLevel=6 cellView=1 cutHeight=1.2 cameraDrift=1 camera.target=0,0.5,-0.6 camera.yaw=28 camera.pitch=30 camera.distance=5.2}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[22, 28, 5.0], [36, 34, 5.6]], "seconds": "beat"}}]}

{@ callout=fixup-parent}{@pl> octreeLevel=7..6}A parent cell gets no colour at all: its albedo, emission, and normal are set to zero, because a parent's light is never baked{<pl}.

{@ highlight=alpha}{@pc> cutHeight=1.2..0.5}It keeps only its coverage, the sum of its children's alpha over eight, so a parent of four leaves holds one half{<pc}.

{@lvp> octreeLevel=6..4}The coarser the level, the fewer of its children exist, and the lower its coverage{<lvp}: {@pc2> cutHeight=0.5..1.6}the light of the coarse levels is rebuilt from the leaves on the GPU, every time the lights change{<pc2}.

## c3-sort | 3d | Sorted by level, each level in one run
{@ demo=cells callout="" image="" panel=code code=sort-cells highlight=key cellArray=1 octreeLevel=-1 cameraDrift=0 camera.target=0,0.9,-0.3 camera.yaw=24 camera.pitch=30 camera.distance=7}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[22, 30], [30, 32]], "seconds": "beat"}}]}

{@ callout=sort-cells}{@sl> octreeLevel=0..3}The cells are then sorted by level first, then by position, {@ highlight=root}with the root at index zero{<sl}.

{@ highlight=remap}{@sl3> octreeLevel=3..5}Each child's index is rewritten to its new place{<sl3}, {@ highlight=runs}{@sl4> octreeLevel=5..7}and each level becomes one run of the array, shown here in its colour{<sl4}.

{@ callout=level-counts}{@sl2> octreeLevel=7..4}The bake also hands over the number of cells in each level, which is how the renderer finds those runs{<sl2}.

## c3-bytes | 3d | Forty-eight bytes a cell, in two records
{@ demo=cells callout="" image="" panel=values cellArray=0 fixupStep=1 focusRow=1 octreeLevel=7 cellView=0 cutHeight=0.3 cameraDrift=0.2 camera.target=-0.01,0.24,-1.71 camera.distance=0.7}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[54, 20, 0.6], [62, 26, 0.8], [56, 22, 0.7]], "seconds": "beat"}}]}

{@ callout=export-octree}{@b1> focusRow=0..1}Each cell is two records: the first holds its eight children's indices, thirty-two bytes, with all ones meaning none{<b1}.

{@ callout=export-data}{@b0> focusRow=2..3}The second is sixteen bytes: its position, packed as eleven, ten, and eleven bits, in that level's own cells{<b0}.

{@bytes> focusRow=4..5}Then its albedo and coverage as four bytes{<bytes}, {@ callout=rgbe9995}{@bytes2> focusRow=6..7}{@ev> cellView=0..3}its emission as three nine-bit numbers sharing one exponent{<ev}{<bytes2}, zero in this room.

{@bytes3> focusRow=8..9}{@nv2> cellView=3..2}Last, its normal as three signed bytes, one per axis{<nv2}{<bytes3}.

## c3-field | 3d | The distance field, exact, in three passes
{@ demo=field callout="" image="" panel=values slice=1 edtPass=0 sliceHeight=0.3 cameraDrift=0}

{@ callout=sdf-edt}{@fh> sliceHeight=0.6..0.3}The bake also measures, for every place in the leaf grid, how far the nearest leaf is, so that shadow rays can later skip through empty space{<fh}.

{@ panel=code code=distance-field highlight=seed}{@fr0> focusRow=1..2}Leaves start at zero and every other place far away, shown white{<fr0}.

{@ed> edtPass=0..3}{@ highlight=z}It then runs an exact distance transform along each z column, {@ highlight=y}then each y, {@ highlight=x}then each x{<ed}, and the slice fills in with distances.

{@ callout=sdf-encode highlight=encode panel=values}{@fr3> focusRow=2..4}Each place keeps one byte: zero at a leaf, else its distance in cells, cut to a whole number, plus one{<fr3}.

{@sh> sliceHeight=0.3..1.0}At the sphere's middle, the field rings each object{<sh}; {@sh2> sliceHeight=1.0..0.6}it will steer the shadow rays{<sh2}.

## c3-upload | 3d | What the GPU keeps
{@ demo=cells callout="" image="" panel=values slice=2 mipLevel=0 sliceHeight=0.0 cellArray=0 octreeLevel=-1 cameraDrift=0.5 camera.target=0,0.4,0 camera.yaw=10 camera.pitch=60 camera.distance=8}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[0, 70], [16, 58]], "seconds": "beat"}}]}

{@ callout=allocate-data}{@us> slice=2..1}The upload turns the two records into two storage buffers, and the field into a three-dimensional texture of single bytes{<us}.

{@ callout=sdf-gpu-disabled}{@us2> slice=1..2}A GPU version of the field exists in the source, compiled out: the bake's own is the one used{<us2}.

{@ callout=instance-texture}{@up> mipLevel=0..3}Each instance adds a colour texture of the octree's size, eight bits a channel, with one mip, a half-size copy, for each level{<up}, and a buffer of four floats per cell.

{@up2> mipLevel=3..0}Here the lab's comes to four megabytes for the finest mip, the cells under four, and the field one{<up2}.

## c3-recap | 3d | Next: light
{@ demo=cells callout="" image="" panel=hidden slice=0 octreeLevel=7 cellView=2 cutHeight=1.0 cameraDrift=1 camera.target=0,0.5,-0.6 camera.yaw=24 camera.pitch=30 camera.distance=5.4}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.distance"], "through": [[32, 5.0], [20, 5.6]], "seconds": "beat"}}]}

{@nv> cellView=2..0}So every leaf now stores what its surface is, its albedo and normal{<nv}, and the field stores how far each place is from one.

{@rcu> cutHeight=1.0..3.2}None of it is light yet; the next chapter lights the leaves{<rcu}.

## c3-sources | 2d | Where to read
sources: fixup-leaf, fixup-parent, sort-cells, export-octree, export-data, rgbe9995, sdf-edt, sdf-encode, allocate-data, instance-texture

These are the lines that finish, pack, measure, and upload the cells.

# 4 | Lighting the cells | 185 s
summary: When the lights change, each leaf gets its direct light, shadowed through the field, and the texture stores it over the dynamic range.
sources: relight-gate, light-cache, max-lights, light-upload, compute-light, light-at-pos, raymarch, area-light, write-texture

## c4-map | 2d | Light passes, on change
diagram: frame
focus: LIGHT, TEX

The cells now hold what each surface is; the light passes give them the light each one receives.

## c4-when | 3d | Only when the data or a light changes
{@ demo=light callout="" image="" panel=values workedLight=0 extraLight=1 lightX=0.9 octreeLevel=7 cellView=5 cutHeight=0.2 focusRow=-1 cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[0, 18, 4.6], [12, 24, 4.4]], "seconds": "beat"}}]}

{@ callout=relight-gate}{@w0> focusRow=-1..0}The light passes do not run every frame: only when the data, its dynamic range, propagation, or bounce setting, or a light changes{<w0}.

{@ callout=light-cache}{@lx> lightX=0.9..-0.6}Each frame the culler compares every light with what it cached: move one, and the cells are lit again, here the floor under its new place{<lx}.

{@ callout=max-lights}{@lx2> lightX=-0.6..0.9}{@w1> focusRow=0..1}At most thirty-two lights take part, each moved into the box's cells{<w1}{<lx2}.

## c4-direct | 3d | A cell's direct light, step by step
{@ demo=light callout="" image="" panel=values workedLight=1 lightStep=0 marchSteps=0 cameraDrift=0.3}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[2, 16], [10, 20]], "seconds": "beat"}}]}

{@ callout=compute-light}{@lh> lightHeight=1.8..1.4}Take this floor cell beside the sphere, ringed, lit by an extra omni light{<lh}.

{@ panel=code code=compute-light highlight=vector}{@st1> lightStep=0..1}The pass finds the direction from the cell to the light{<st1}, {@ callout=omni-atten highlight=atten panel=values}{@st2> lightStep=1..2}{@lxa> lightX=0.9..0.5}and how much it falls off over that distance{<lxa}{<st2}.

{@ callout=light-at-pos}{@st3> lightStep=2..3}{@lxb> lightX=0.5..0.9}A cell facing away gets nothing; otherwise the light is weighed by its cosine to the cell's normal{<lxb}{<st3}.

{@ panel=code code=compute-light highlight=result}{@st4> lightStep=3..4}Times the cell's albedo, plus its emission, that is the light the cell sends back out{<st4}.

## c4-shadow | 3d | The shadow ray steps through the distance field
{@ demo=light callout="" image="" panel=code code=raymarch highlight=loop workedLight=1 lightStep=4 marchSteps=0 cameraDrift=0.3}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[0, 16, 4.4], [8, 20, 4.0]], "seconds": "beat"}}]}

{@ callout=raymarch}{@m0> marchSteps=0..1}With shadows on, a ray starts at the light's end of the segment and walks toward the cell through the distance field{<m0}.

{@ma> marchSteps=1..4}{@ highlight=read}At each step it reads how far the nearest surface is, the circle, {@ highlight=step}and jumps that far, rounded down to whole cells along its main axis{<ma}.

{@ highlight=solid}{@ma2> marchSteps=4..40}Here the ray reaches the sphere's cells and reads a solid one, so the cell is in shadow and gets none of this light{<ma2}.

{@ highlight=partial}{@lz> lightZ=-0.4..-0.1}Near a surface the field reads less than one cell, and the light is only dimmed: the shadow's edge is a little soft{<lz}.

## c4-area | 3d | The panel's area light has no shadow
{@ demo=light callout="" image="" panel=values workedLight=1 lightStep=4 extraLight=0 cameraDrift=0.5 camera.target=-1.0,1.2,-0.5 camera.distance=5.6}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[0, 16], [10, 22]], "seconds": "beat"}}]}

{@ callout=area-light}{@ls> lightStep=1..4}The ceiling panel is an area light: its light is an integral over its whole rectangle, here a closed-form fit of how much of the rectangle the cell sees{<ls}.

{@cutA> cutHeight=0.2..1.2}That path never marches the field: the panel lights the cells under the sphere as if the sphere were not there{<cutA}.

{@ callout=ltc}{@cutB> cutHeight=1.2..0.2}The integral is used without the division by two pi that direct lighting applies, read from the code{<cutB}.

## c4-all | 3d | Every leaf, lit
{@ demo=light callout="" image="" panel=hidden workedLight=0 extraLight=0 octreeLevel=7 cellView=5 cutHeight=0.2 cameraDrift=1 camera.target=0,0.6,-0.4 camera.yaw=18 camera.pitch=34 camera.distance=7}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[12, 32, 6.6], [28, 38, 7.2]], "seconds": "beat"}}]}

{@cu> cutHeight=0.2..3.2}Every leaf runs the same pass, sixty-four to a GPU group{<cu}, and the room's cells take on the panel's light.

{@cv> cellView=5..1}Where the panel sees a cell it is bright, and cells it cannot see stay dark{<cv}, {@cv2> cellView=1..5}waiting for the bounce{<cv2}.

## c4-write | 3d | The texture stores light over the dynamic range
{@ demo=light callout="" image="" panel=values workedLight=0 extraLight=0 dynamicRange=2 octreeLevel=7 cellView=4 cutHeight=0.2 cameraDrift=0.5 camera.target=0,0.1,-0.3 camera.yaw=14 camera.pitch=48 camera.distance=3.4}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[8, 50], [20, 44]], "seconds": "beat"}}]}

{@ callout=write-texture focusRow=2}{@wv> cellView=5..4}The write pass stores each cell's light in its texel, eight bits a channel, after dividing by the dynamic range, two by default{<wv}.

{@dr> dynamicRange=2..1}{@drr> focusRow=2..4}So light up to twice white survives; at a range of one, the bright cells under the panel clip to white, and the count of clipped leaves climbs{<drr}{<dr}.

{@dr2> dynamicRange=1..2}The alpha channel stores the coverage, and texels with no cell stay zero, transparent{<dr2}.

{@ callout=unused-push}{@wc> cutHeight=0.2..1.0}The pass still carries an anisotropy setting no line reads: this version stores no direction in its light{<wc}.

## c4-recap | 3d | Next: the bounce
{@ demo=light callout="" image="" panel=hidden workedLight=0 extraLight=0 octreeLevel=7 cellView=5 cutHeight=3.2 cameraDrift=1 camera.target=0,0.6,-0.4 camera.yaw=18 camera.pitch=34 camera.distance=7}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.distance"], "through": [[26, 6.6], [14, 7.2]], "seconds": "beat"}}]}

{@rl> cutHeight=3.2..0.4}So every leaf now holds the direct light it sends out, shadowed through the field for punctual lights{<rl}.

{@rl2> cutHeight=0.4..3.2}That light reaches only what a light sees; the next chapter lets the cells light each other{<rl2}.

## c4-sources | 2d | Where to read
sources: relight-gate, light-cache, max-lights, light-upload, compute-light, light-at-pos, raymarch, area-light, write-texture

These are the light pass's lines, from the gate to the texture.

# 5 | The second bounce and the mips | 160 s
summary: The coarse levels are filled from their children, then six cones from every leaf gather the light around it.
sources: update-mipmaps, pass-order, second-bounce, bounce-cones, bounce-march, bounce-restore

## c5-map | 2d | The texture reads itself back
diagram: frame
focus: LIGHT, TEX

One pass of light reaches only what the lights see; a second pass reads the texture the first wrote, so the cells light each other.

## c5-mips | 3d | A parent's light from its children
{@ demo=mips callout="" image="" panel=code code=update-mipmaps highlight=children slice=2 mipLevel=0 workedParent=1 cameraDrift=0.3}
{@ animate=[{"path": {"params": ["camera.yaw"], "through": [[6], [-6], [4]], "seconds": "beat"}}]}

{@ callout=update-mipmaps}{@mh> sliceHeight=0.4..0.0}A cone widens as it travels, so it must read coarser copies of the light; the bake left those levels empty, so they are filled first{<mh}.

{@mm> mipLevel=0..1}Each parent sums its children's light{<mm}, {@ highlight=divisor panel=values}{@mm15> mipLevel=1..2}and divides by a number between eight and how many children it has, set by the propagation setting{<mm15}.

{@mm2> mipLevel=2..4}Level by level, each mip holds a coarser copy of the light{<mm2}, {@mm3> mipLevel=4..7}down to one texel for the whole box{<mm3}.

## c5-prop | 3d | Propagation sets the divisor
{@ demo=mips callout="" image="" panel=values slice=2 mipLevel=2 workedParent=1 propagation=0.5 focusRow=3 cameraDrift=0.3}
{@ animate=[{"path": {"params": ["camera.yaw"], "through": [[-4], [6]], "seconds": "beat"}}]}

{@pr> propagation=0.5..0}At a propagation of zero, the divisor is always eight, so empty children count as black and the coarse levels darken{<pr}.

{@pr2> propagation=0..1}At one, only the children that exist are averaged, and the coarse levels keep their brightness{<pr2}.

{@pr3> propagation=1..0.5}The default, one half, sits between, so a parent of four leaves divides by six{<pr3}, {@pf> focusRow=3..5}a choice between light that fades with distance and light that spreads too far{<pf}.

## c5-cones | 3d | Six cones from every leaf
{@ demo=bounce callout="" image="" panel=code code=second-bounce highlight=frame workedBounce=1 conesShown=0 coneSteps=6 cameraDrift=0.3}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[34, 28], [48, 32], [40, 30]], "seconds": "beat"}}]}

{@ callout=second-bounce}{@cs0> coneSteps=2..6}Take this floor cell near the red wall, ringed{<cs0}.

{@co> conesShown=0..1}{@cz> coneSteps=6..4}{@ highlight=cones}The bounce builds a frame around its normal and sends one cone straight up it, weighted a quarter{<cz}{<co}.

{@ callout=bounce-cones}{@co2> conesShown=1..6}Five more lean sixty degrees out, at fifteen hundredths each, so the six weigh one in all{<co2}; {@co3> coneSteps=6..3}each cone opens thirty degrees from its axis{<co3}.

## c5-march | 3d | Each cone widens and reads coarser mips
{@ demo=bounce callout="" image="" panel=code code=second-bounce highlight=march workedBounce=1 conesShown=1 coneSteps=0 cameraDrift=0.3}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.distance"], "through": [[40, 2.4], [30, 3.2]], "seconds": "beat"}}]}

{@ callout=bounce-march}{@one> coneSteps=0..1}A cone starts a cell and a half from the cell's centre, clear of the cell itself{<one}.

{@ run=marchCone run.seconds=0.7 run.from=1 run.to=6}Each step it moves on by half its width, so it widens as it goes, {@ highlight=lod}and reads the mip whose texels match that width.

{@ highlight=accum panel=values} Each sample is laid behind what the cone already saw, until it is nineteen-twentieths opaque or runs out of distance.

{@ stop=marchCone}{@all> conesShown=1..6}{@far> coneSteps=6..11}The last cone here takes eleven steps and still ends only two thirds opaque: the room's cells are thin shells around open air{<far}{<all}.

## c5-restore | 3d | The bounce is added, and everything rebuilt
{@ demo=light callout="" image="" panel=code code=second-bounce highlight=restore workedLight=0 extraLight=0 octreeLevel=7 cellView=4 cutHeight=0.2 bounces=1 cameraDrift=1 camera.target=-1.4,0.4,0.2 camera.yaw=40 camera.pitch=34 camera.distance=4.2}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[34, 32, 4.0], [48, 36, 4.6]], "seconds": "beat"}}]}

{@ callout=bounce-restore}{@rc> cutHeight=0.2..0.8}Each cone's light is weighted, times the cell's albedo for the bounce off it, times the dynamic range to undo the texture's division, and added to the cell's direct light{<rc}.

{@ callout=pass-order}{@bo> bounces=1..2}Then the mips are filled again and the texture written again, and the floor cells beside the red wall brighten{<bo}.

{@bo2> bounces=2..1}Only the leaves trace{<bo2}, {@bo3> bounces=1..2}and only once: there is no third bounce{<bo3}.

## c5-recap | 3d | Next: what moves
{@ demo=light callout="" image="" panel=hidden workedLight=0 extraLight=0 octreeLevel=7 cellView=4 cutHeight=3.2 bounces=2 cameraDrift=1 camera.target=0,0.6,-0.4 camera.yaw=18 camera.pitch=34 camera.distance=7}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.distance"], "through": [[28, 6.6], [16, 7.2]], "seconds": "beat"}}]}

{@lvr> octreeLevel=7..5}{@rcv> cutHeight=3.2..1.0}So the texture now holds two bounces of light, at every level from five-centimetre cells to the whole box{<rcv}{<lvr}.

{@lvr2> octreeLevel=5..7}{@rcv2> cutHeight=1.0..3.2}All of it was baked from objects that stand still; the next chapter adds one that moves{<rcv2}{<lvr2}.

## c5-sources | 2d | Where to read
sources: update-mipmaps, pass-order, second-bounce, bounce-cones, bounce-march, bounce-restore

These are the mip and bounce passes, and the order they run in.

# 6 | Moving objects | 165 s
summary: An object marked Dynamic is drawn from six sides every frame, lit, shrunk, and plotted into the same texture.
sources: gi-mode-flags, dynamic-list, dynamic-oversample, dynamic-views, material-outputs, dynamic-lighting, shrink, plot-blend, dynamic-clear

## c6-map | 2d | Dynamic objects write into the texture
diagram: frame
focus: DYN, TEX

The bake knows only what stood still, and baking again every frame would be far too slow; so an object marked Dynamic is drawn into the texture again every frame instead.

## c6-mode | 3d | Dynamic objects are not baked
{@ demo=dynamic callout="" image="" panel=values dynamicVoxels=0 dynamicView=-1 dynamicX=0 cameraDrift=0.5}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.distance"], "through": [[50, 1.8], [60, 2.2]], "seconds": "beat"}}]}

{@ callout=gi-mode-flags}{@dv0> dynamicMip=1..0}Here the teapot's GI mode is Dynamic, so the bake leaves it out, and the lab's room was baked without it: no cells, no light{<dv0}.

{@ callout=dynamic-list}{@dv1> dynamicVoxels=0..1}Every frame the room's VoxelGI is in view, each of its dynamic objects is drawn into the texture, moving or not{<dv1}.

{@mv> dynamicX=0..0.6}The points are the texels it wrote, drawn brighter than stored, in the light they received{<mv}; {@mv2> dynamicX=0.6..0}the lab draws them as points, not in its shading{<mv2}.

## c6-views | 3d | Six views, sixteen texels a cell
{@ demo=dynamic callout="" image="" panel=values dynamicVoxels=0 dynamicView=0 shrinkLevel=0 dynamicX=0 cameraDrift=0.5}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[48, 18], [62, 24]], "seconds": "beat"}}]}

{@ callout=dynamic-oversample}{@vx> dynamicX=0.3..0}The object's bounding box is drawn at sixteen texels per cell, fine enough that shrinking it later measures how much of each cell it covers{<vx}.

{@ callout=dynamic-views}{@vw> dynamicView=0..5}It is drawn six times, in flat orthographic views along each axis both ways, so no side is missed{<vw}.

{@ callout=material-outputs}{@vw0> dynamicView=5..0}Each view records the material's albedo, normal, and emission, and the depth of every texel{<vw0}.

{@ callout=dynamic-lighting}{@vl> shrinkLevel=0..1}A texel's depth gives its place in the grid, and it is lit like a cell, its shadows from the baked field alone{<vl}.

## c6-shrink | 3d | Shrunk two by two, plotted at each mip
{@ demo=dynamic callout="" image="" panel=code code=dynamic-object highlight=shrink dynamicVoxels=0 dynamicView=0 shrinkLevel=0 cameraDrift=0.5}
{@ animate=[{"path": {"params": ["camera.yaw"], "through": [[54], [64], [56]], "seconds": "beat"}}]}

{@ callout=shrink}{@sk> shrinkLevel=0..3}The lit view is halved, two by two texels into one, from sixteen texels a cell to eight, four, and two{<sk}.

{@ highlight=plot}{@sk2> shrinkLevel=3..4}At one texel a cell, each texel is plotted into the texture's finest mip{<sk2}.

{@ callout=plot-blend}{@pv> dynamicVoxels=0..1}Its depth falls between two cells along the view, so it is split between them by distance, and blended over what is there{<pv}.

{@sk3> shrinkLevel=4..11}{@pm> dynamicMip=0..3}Each further halving plots into the next mip{<pm}{<sk3}, with the mip pass's divisor.

## c6-mips | 3d | The plotted voxels, mip by mip
{@ demo=dynamic callout="" image="" panel=values dynamicVoxels=1 dynamicView=-1 dynamicMip=0 dynamicX=0 cameraDrift=0.5}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[46, 18, 1.8], [58, 24, 2.4]], "seconds": "beat"}}]}

{@mp1> dynamicMip=0..1}{@mx1> dynamicX=0..0.3}At the finest mip the teapot is drawn in five-centimetre cells; one mip up, in ten{<mx1}{<mp1}.

{@mp2> dynamicMip=1..3}{@mx2> dynamicX=0.3..0}Each mip halves the cells again, and their light is blended with the divisor{<mx2}{<mp2}, {@mp3> dynamicMip=3..0}which is what the wider cones will read{<mp3}.

## c6-cost | 3d | Rewritten every frame, never bounced
{@ demo=dynamic callout="" image="" panel=values dynamicVoxels=1 dynamicView=-1 dynamicMip=0 dynamicX=0 cameraDrift=0.5}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[50, 20, 1.6], [64, 26, 2.0]], "seconds": "beat"}}]}

{@ callout=dynamic-clear}{@cx> dynamicX=0..0.3}Before the plot, the whole texture is cleared and the static light written back from the float buffer{<cx}.

{@dm> dynamicMip=0..3}So the dynamic texels exist only for this frame, at every mip{<dm}; {@cx3> dynamicX=0.3..0.5}and since the bounce ran in the light passes before them, nothing bounces off them, and they cast no shadow{<cx3}.

{@dm2> dynamicMip=3..0}Here the cost is six draws and seventy-two compute dispatches per object per frame{<dm2}, {@ callout=dynamic-light-count}{@cx2> dynamicX=0.5..0}with a light count not held to the buffer's thirty-two{<cx2}.

## c6-recap | 3d | Next: reading the texture
{@ demo=dynamic callout="" image="" panel=hidden dynamicVoxels=1 dynamicView=-1 dynamicX=0 cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.distance"], "through": [[48, 1.8], [62, 2.4]], "seconds": "beat"}}]}

{@mvr> dynamicX=0..0.4}{@mvr3> dynamicMip=0..1}So a moving object shows up in the same texture, a frame at a time, lit directly but never bounced{<mvr3}{<mvr}.

{@mvr2> dynamicX=0.4..0}{@mvr4> dynamicMip=1..0}Static or moving, everything ends in that one texture, which the next chapter reads back{<mvr4}{<mvr2}.

## c6-sources | 2d | Where to read
sources: gi-mode-flags, dynamic-list, dynamic-oversample, dynamic-views, dynamic-lighting, shrink, plot-blend, dynamic-clear

These are the dynamic path's lines.

# 7 | Reading it per pixel | 185 s
summary: The pre-pass tags each pixel with its VoxelGIs; the screen pass moves it into the box and traces four cones and a reflection cone.
sources: pairing-two, instance-ubo, tag-write, process-gi, voxel-gi-compute, edge-blend, cones-low, cone-trace-45, cones-high, cone-trace, spec-cone, range-restore

## c7-map | 2d | The screen pass reads the texture
diagram: frame
focus: PRE, CONE, TEX

With the texture full of light, the question becomes how a pixel reads it cheaply; every frame, the screen pass answers it.

## c7-tag | 3d | Each pixel knows its VoxelGIs
{@ demo=pixel callout="" image="" panel=values workedPixel=1 conesShown=0 specCone=0 focusRow=-1 cameraDrift=0.5}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[36, 14], [46, 18]], "seconds": "beat"}}]}

{@ callout=pairing-two}{@ta> pixelElevation=10..30}Each object keeps the first two VoxelGIs it overlaps{<ta}.

{@ callout=tag-write}{@t0> focusRow=-1..0}{@ta2> pixelAzimuth=0..-30}The depth pre-pass writes their indices into every pixel it draws, two bytes, with two hundred fifty-five meaning none{<ta2}{<t0}.

{@ callout=instance-ubo}{@ta3> pixelAzimuth=-30..0}{@ta4> pixelElevation=30..10}Up to eight VoxelGIs in view share one buffer of their settings; this sphere's pixels name the room's only one{<ta4}{<ta3}.

## c7-space | 3d | Into the box's cells
{@ demo=pixel callout="" image="" panel=values workedPixel=1 conesShown=0 pixelAzimuth=0 focusRow=1 cameraDrift=0.3}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.distance"], "through": [[40, 3.4], [32, 3.0]], "seconds": "beat"}}]}

{@ callout=voxel-gi-compute}{@el> pixelElevation=30..10}Take this pixel on the sphere's side, ringed, facing the red wall{<el}.

{@az> pixelAzimuth=0..-40}Its position and normal are moved into the box's cells{<az}, and a pixel outside the box gets nothing from it.

{@ callout=edge-blend}{@s2> focusRow=1..2}{@az2> pixelAzimuth=-40..0}An edge fade weighs it: one at the box's centre, zero at its faces{<az2}{<s2}.

{@el2> pixelElevation=10..40}The fade exists to blend two overlapping boxes; with one box, the sums are divided by that same fade, so it cancels, and only a pixel exactly on a face loses the box's light{<el2}.

## c7-low | 3d | Four cones at forty-five degrees
{@ demo=pixel callout="" image="" panel=values workedPixel=1 conesShown=0 coneSteps=16 focusRow=3 cameraDrift=0.3}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[40, 16], [30, 22]], "seconds": "beat"}}]}

{@ callout=cones-low}{@c4> conesShown=0..4}At the default quality, four cones leave the pixel at forty-five degrees from its normal, a quarter each{<c4}.

{@ callout=cone-trace-45}{@cs> coneSteps=0..2}Such a cone's radius equals its distance, so each step doubles the distance{<cs}, {@cs2> coneSteps=2..6}and reads one mip coarser, a texel twice as wide, which already blends everything in its footprint{<cs2}.

{@la> pixelAzimuth=0..-25}The samples are laid front to back, until the cone is opaque, its alpha one, or leaves the box{<la}.

## c7-high | 3d | Six cones at High
{@ demo=pixel callout="" image="" panel=values workedPixel=1 conesShown=6 coneSteps=16 quality=0 cameraDrift=0.3}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[34, 18], [44, 20]], "seconds": "beat"}}]}

{@ callout=cones-high}{@hq> quality=0..1}At High quality, the pixel traces the bounce's six cones instead, each thirty degrees from its axis to its edge{<hq}.

{@ callout=cone-trace}{@hq2> coneSteps=16..4}Each takes smaller steps, half its width at a time{<hq2}, {@hq3> coneSteps=4..16}so they take more samples, each from a finer mip{<hq3}.

{@hq4> quality=1..0}At the default, four wide cones do the work{<hq4}.

## c7-spec | 3d | One more cone for the reflection
{@ demo=pixel callout="" image="" panel=values workedPixel=1 conesShown=0 specCone=1 quality=0 sphereRoughness=0.5 cameraDrift=0.3}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[38, 16], [30, 20]], "seconds": "beat"}}]}

{@ callout=spec-cone}{@sa> pixelAzimuth=0..-20}The reflection gets one cone along the mirror direction{<sa}.

{@ro> sphereRoughness=0.5..0.15}Its width follows the roughness, squared first: a smooth sphere's cone is narrow and travels far{<ro}, {@ro2> sphereRoughness=0.15..0.9}a rough one's opens seventy degrees from its axis{<ro2}.

{@ro3> sphereRoughness=0.9..0.5}{@sa2> pixelAzimuth=-20..0}The sphere's reflection in the room follows it, from sharp to blurred{<sa2}{<ro3}.

## c7-scale | 3d | The range restored, the coverage kept
{@ demo=pixel callout="" image="" panel=values workedPixel=1 conesShown=4 specCone=1 interior=0 cameraDrift=0.3}
{@ animate=[{"path": {"params": ["camera.yaw"], "through": [[36], [44]], "seconds": "beat"}}]}

{@ callout=range-restore}{@sc> pixelAzimuth=0..-15}The cones' light is multiplied back by the dynamic range that the texture divided it by, and by the energy{<sc}.

{@ panel=code code=voxel-gi-compute highlight=interior}{@in> interior=0..1}Their alpha says how much of the view they found filled; with interior set, it is forced to one{<in}.

{@ panel=values}{@in2> interior=1..0}That alpha travels with the light into the buffers{<in2}.

## c7-quirk | 3d | A check that is always true
{@ demo=pixel callout="" image="" panel=code code=voxel-gi-compute highlight=space workedPixel=1 conesShown=4 specCone=0 cameraDrift=0.3}
{@ animate=[{"path": {"params": ["camera.yaw"], "through": [[40], [32]], "seconds": "beat"}}]}

{@ callout=normal-length-quirk}{@el3> pixelElevation=10..25}Before any of this, the pass checks that the pixel's normal is valid, with a test that is always true: a vector's length method in this shading language returns its component count, three{<el3}.

## c7-recap | 3d | Next: into the image
{@ demo=pixel callout="" image="" panel=hidden workedPixel=1 conesShown=4 specCone=1 cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.distance"], "through": [[30, 3.4], [44, 3.8]], "seconds": "beat"}}]}

{@az3> pixelAzimuth=0..30}So every pixel traces four cones and a reflection cone through the texture{<az3}, and keeps how much of each view they filled.

{@az4> pixelAzimuth=30..0}Those two results go into buffers, which the opaque pass turns into colour{<az4}.

## c7-sources | 2d | Where to read
sources: pairing-two, tag-write, process-gi, voxel-gi-compute, edge-blend, cones-low, cone-trace-45, cones-high, cone-trace, spec-cone, range-restore

These are the screen pass's lines, from the tag to the scaled cones.

# 8 | Into the final image | 160 s
summary: The opaque pass mixes the environment toward VoxelGI's light by its alpha; one pixel followed back; three takeaways.
sources: gi-buffer-mix, forward-gi-transparent, forward-cones, fog-inject, debug-noop, bias-mismatch

## c8-map | 2d | The opaque pass
diagram: frame
focus: CONE, OPAQUE

The screen pass leaves two buffers per pixel, an ambient and a reflection; the opaque pass turns them into colour.

## c8-mix | 3d | Mixed with the environment by alpha
{@ demo=compose callout="" image="" panel=values viewMode=0 interior=0 focusRow=1 cameraDrift=0.5}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch"], "through": [[36, 14], [46, 18]], "seconds": "beat"}}]}

{@ callout=gi-buffer-mix}{@vm> viewMode=0..1}{@pa1> pixelAzimuth=0..-30}The ambient buffer holds the cones' light{<pa1}{<vm}, {@vm2> viewMode=1..3}and its alpha, how much of the view they filled{<vm2}.

{@vm3> viewMode=3..0}{@x1> focusRow=1..2}A cone that leaves the box finds nothing, so where the alpha is low the pass mixes toward the environment's own ambient light, and the sky's reflection the same way{<x1}{<vm3}.

{@in> interior=0..1}{@x2> focusRow=2..3}So the sky still lights what the cones saw empty, here through the room's open top and front, unless interior is set{<x2}{<in}.

{@in2> interior=1..0}{@x3> focusRow=3..7}{@pa3> pixelAzimuth=-30..0}Then ambient occlusion, the albedo, screen-space effects, and the environment's reflectance follow, as for any ambient light{<pa3}{<x3}{<in2}.

## c8-others | 3d | Transparent surfaces and fog read it too
{@ demo=room callout="" image="" panel=hidden voxelgiMix=1 cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[14, 16, 6.6], [28, 20, 6.0]], "seconds": "beat"}}]}

{@ callout=forward-gi-transparent}{@ov> viewMode=0..1}{@oq> quality=0..1}Transparent surfaces, none in this room, cannot use the buffers; they trace their own four cones in the forward pass, at every quality{<oq}{<ov}.

{@ callout=fog-inject}{@ov2> viewMode=1..0}{@oq2> quality=1..0}Volumetric fog reads every mip of the texture at each point of its grid, with no cone at all{<oq2}{<ov2}.

{@ callout=debug-noop}{@ov3> voxelgiMix=1..0.5}And the editor's lighting view forgets to apply the dynamic range, so at the default range it shows the light at half{<ov3}.

## c8-follow | 3d | One pixel, followed back
{@ demo=pixel callout="" image="" panel=hidden workedPixel=1 conesShown=4 specCone=0 coneSteps=16 cameraDrift=0.3}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[36, 16, 3.2], [24, 22, 3.6], [32, 18, 3.0]], "seconds": "beat"}}]}

{@fe> pixelElevation=10..20}Follow this pixel on the sphere back through it all{<fe}.

{@fc> coneSteps=16..2}Its cones read the texture's mips around it{<fc}, {@ demo=mips slice=2 mipLevel=0 workedParent=1 panel=hidden}{@fm> mipLevel=0..3}the mips the mip pass built from the leaves{<fm}.

{@ demo=bounce workedBounce=1 conesShown=0 coneSteps=6 panel=hidden}{@fb> conesShown=0..6}{@fbs> coneSteps=1..6}Those leaves hold the panel's direct light and one bounce, gathered by six cones of their own{<fbs}{<fb}.

{@ demo=octree workedTriangle=1 workedLeaf=1 leafColumns=0 octreeLevel=-1 camera.target=-0.01,0.24,-1.71 camera.distance=0.4 panel=hidden}{@fl> leafColumns=0..16}{@flv> descentLevel=5..7}And each leaf is the average of the samples the bake took from the room's triangles{<flv}{<fl}.

## c8-recap | 3d | Three takeaways
{@ demo=room callout="" image="" panel=hidden voxelgiMix=0 cameraDrift=1}
{@ animate=[{"path": {"params": ["camera.yaw", "camera.pitch", "camera.distance"], "through": [[16, 16, 6.8], [26, 20, 6.2], [20, 18, 6.4]], "seconds": "beat"}}]}

{@m1> voxelgiMix=0..1}{@m1v> viewMode=0..3}First, the data: a sparse octree, baked once, poured into a dense texture with one mip per level{<m1v}{<m1}.

{@m2> voxelgiMix=1..0.4}{@m2v> viewMode=3..1}Second, the light: passes that run when lights change, then cones from every pixel that read wider mips as they go{<m2v}{<m2}.

{@m3> voxelgiMix=0.4..1}{@m3v> viewMode=1..0}Third, the trade: a bake of the static scene, moving objects only drawn in each frame, five-centimetre cells, cones that see the room only as coarse averages, and no shadows from area lights, for a few cone samples per pixel{<m3v}{<m3}.

## c8-sources | 2d | Where to read
sources: gi-buffer-mix, forward-gi-transparent, forward-cones, fog-inject, debug-noop, bias-mismatch

These are the composite's lines and the other readers.

Every source this video cites is in its source-reading guide, in the order the frame runs it.
