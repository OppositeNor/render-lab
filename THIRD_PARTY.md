# Third-party material

Everything in this repository written for Render Lab (the Rust lab and its WGSL shaders) is released under the
[MIT License](LICENSE), copyright © 2026 OppositeNor.

The following are **not** covered by that license; they keep their own terms.

| Material | Where | Terms |
|---|---|---|
| DejaVu fonts | `packages/rlstd/assets/fonts/DejaVu*` | DejaVu fonts license ([`DejaVu-LICENSE.txt`](packages/rlstd/assets/fonts/DejaVu-LICENSE.txt)) |
| Source Han Sans CN | `packages/rlstd/assets/fonts/SourceHanSans*` | SIL Open Font License 1.1 ([`SourceHanSans-LICENSE.txt`](packages/rlstd/assets/fonts/SourceHanSans-LICENSE.txt)) |
| Utah teapot mesh (Cem Yuksel's 2026 version of Martin Newell's teapot) | `packages/rlstd/assets/models/utah_teapot.obj` | Free for any use, including commercial, provided as is. The terms ask that the model and substantially derived versions be identified as a Utah Teapot (or Modified Utah Teapot) and credited as the historic Utah Teapot developed at the University of Utah. The lab only scales and positions it, so it is identified here as the Utah Teapot. |
| MiTeX's Typst scope for LaTeX (the `specs/` of the `mitex` Typst package 0.2.6) | `packages/rlstd/assets/mitex/specs/` | Apache License 2.0 ([`packages/rlstd/assets/mitex/LICENSE`](packages/rlstd/assets/mitex/LICENSE)) |
| New Computer Modern and New Computer Modern Math, embedded through the `typst-assets` crate | `vendor/typst-assets/` (not committed) | SIL Open Font License 1.1, which places no conditions on the formulas rendered with them |
| "Tree Lined Driveway" HDRI from Poly Haven (photographed by Dimitrios Savva, processed by Jarod Guest) | `packages/rlstd/assets/environments/tree_lined_driveway_4k.hdr` | CC0 1.0 (public domain); no attribution required. The lab clips its sun and scales it when it loads it (its resource pipeline); the file is unchanged. |
| Sounds from soundza (Orange Lake, LLC): the music loop "Bright Choir Loop in Em" (re-encoded as Ogg Vorbis) and the "UI Button Click" effect | `packages/rlstd/assets/audio/` ([`README.md`](packages/rlstd/assets/audio/README.md)) | soundza Standard License ([`LICENSES/soundza-Standard-License.txt`](LICENSES/soundza-Standard-License.txt)), **not** the MIT License. They are included only as the homepage's music and click sound in Render Lab. You may not take them out of the lab to use, share, or sell as sounds of your own (for example in a sample pack or a sound library), claim them as yours, or use them to train or develop AI or machine-learning models. To use them in your own work, get them from soundza.com under its license. |
| The colour schemes the themes follow: One Dark Pro (Binaryify, after Atom's One Dark), Tokyo Night (enkia), and Visual Studio Code's Dark Modern and Dark+ (Microsoft) | `themes/` | The files are written for Render Lab and only take colour values from these schemes, which are published under the MIT License. The names identify the schemes the themes follow; Render Lab is not affiliated with or endorsed by their authors. |
| Rust crates | `vendor/` (not committed; recreated by `setup.sh`) | Each crate's own license, mostly MIT or Apache-2.0 |
| Open 3D Engine shader code, ported to WGSL and Rust | `packages/o3de_lighting/shaders/standard_lighting.wgsl`, `packages/o3de_lighting/src/o3de/lighting/brdf.rs`, and the pseudo-code in `packages/o3de_lighting/pseudocode/` | Copyright Contributors to the Open 3D Engine Project. O3DE is licensed under Apache-2.0 or MIT at the user's option; Render Lab uses it under the MIT License ([`LICENSES/O3DE-MIT.txt`](LICENSES/O3DE-MIT.txt)). |
| CubeMapGen's cubemap filtering, as bundled in O3DE, ported to Rust | `packages/o3de_lighting/src/o3de/lighting/ibl.rs` | Copyright (c) 2011, Advanced Micro Devices, Inc. Modified BSD License ([`LICENSES/CubeMapGen-BSD.txt`](LICENSES/CubeMapGen-BSD.txt)); O3DE's changes to it under the MIT License as above. |
| Godot Engine's SDFGI code, ported to Rust and WGSL | `packages/godot_sdfgi/src/godot/sdfgi/`, `packages/godot_sdfgi/shaders/`, and the pseudo-code in `packages/godot_sdfgi/pseudocode/` | Copyright (c) 2014-present Godot Engine contributors; Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur. MIT License ([`LICENSES/Godot-MIT.txt`](LICENSES/Godot-MIT.txt)). |
| Godot Engine's VoxelGI code, ported to Rust and WGSL | `packages/godot_voxelgi/src/godot/voxelgi/`, `packages/godot_voxelgi/shaders/`, and the pseudo-code in `packages/godot_voxelgi/pseudocode/` | Copyright (c) 2014-present Godot Engine contributors; Copyright (c) 2007-2014 Juan Linietsky, Ariel Manzur. MIT License ([`LICENSES/Godot-MIT.txt`](LICENSES/Godot-MIT.txt)). |

Investigations of engines whose source may not be published are not part of this repository; the investigations of the
Open 3D Engine and the Godot Engine are, because their licenses allow it. Each of those packages carries its notices and
a copy of its licenses (`packages/godot_sdfgi/LICENSE-Godot.txt`, `packages/godot_voxelgi/LICENSE-Godot.txt`, `packages/o3de_lighting/LICENSE-O3DE.txt` and
`LICENSE-CubeMapGen.txt`), which the lab shows on its selection page and at the end of its videos, and which go with
the built package and beside each video (`NOTICE.txt`). Engine names are trademarks of their owners; Render Lab is not
affiliated with or endorsed by any engine's maker.
