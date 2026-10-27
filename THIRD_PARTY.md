# Third-party material

Everything in this repository written for Render Lab (the Rust lab and its WGSL shaders) is released under the
[MIT License](LICENSE), copyright © 2026 OppositeNor.

The following are **not** covered by that license; they keep their own terms.

| Material | Where | Terms |
|---|---|---|
| DejaVu fonts | `assets/fonts/DejaVu*` | DejaVu fonts license ([`DejaVu-LICENSE.txt`](assets/fonts/DejaVu-LICENSE.txt)) |
| Source Han Sans CN | `assets/fonts/SourceHanSans*` | SIL Open Font License 1.1 ([`SourceHanSans-LICENSE.txt`](assets/fonts/SourceHanSans-LICENSE.txt)) |
| Utah teapot mesh (Cem Yuksel's 2026 version of Martin Newell's teapot) | `assets/models/utah_teapot.obj` | Free for any use, including commercial, provided as is. The terms ask that the model and substantially derived versions be identified as a Utah Teapot (or Modified Utah Teapot) and credited as the historic Utah Teapot developed at the University of Utah. The lab only scales and positions it, so it is identified here as the Utah Teapot. |
| Formula images, rendered with KaTeX (MIT License) in KaTeX's fonts | `src/o3de/lighting/formulas/*.png` | The fonts are under the SIL Open Font License 1.1, which places no conditions on text rendered with them; the formulas themselves are Render Lab's. |
| "Tree Lined Driveway" HDRI from Poly Haven (photographed by Dimitrios Savva, processed by Jarod Guest) | `assets/environments/tree_lined_driveway_4k.hdr` | CC0 1.0 (public domain); no attribution required. The lab clips its sun and scales it, at run time; the file is unchanged. |
| Sounds from soundza (Orange Lake, LLC): the music loop "Bright Choir Loop in Em" (re-encoded as Ogg Vorbis) and the "UI Button Click" effect | `assets/audio/` ([`README.md`](assets/audio/README.md)) | soundza Standard License ([`LICENSES/soundza-Standard-License.txt`](LICENSES/soundza-Standard-License.txt)), **not** the MIT License. They are included only as the homepage's music and click sound in Render Lab. You may not take them out of the lab to use, share, or sell as sounds of your own (for example in a sample pack or a sound library), claim them as yours, or use them to train or develop AI or machine-learning models. To use them in your own work, get them from soundza.com under its license. |
| Rust crates | `vendor/` (not committed; recreated by `setup.sh`) | Each crate's own license, mostly MIT or Apache-2.0 |
| Open 3D Engine shader code, ported to WGSL and Rust | `src/o3de/lighting/shaders/standard_lighting.wgsl`, `src/o3de/lighting/brdf.rs`, and the pseudo-code in `src/o3de/lighting/pseudocode/` | Copyright Contributors to the Open 3D Engine Project. O3DE is licensed under Apache-2.0 or MIT at the user's option; Render Lab uses it under the MIT License ([`LICENSES/O3DE-MIT.txt`](LICENSES/O3DE-MIT.txt)). |
| CubeMapGen's cubemap filtering, as bundled in O3DE, ported to Rust | `src/o3de/lighting/ibl.rs` | Copyright (c) 2011, Advanced Micro Devices, Inc. Modified BSD License ([`LICENSES/CubeMapGen-BSD.txt`](LICENSES/CubeMapGen-BSD.txt)); O3DE's changes to it under the MIT License as above. |

Investigations of engines whose source may not be published are not part of this repository; the investigation of the
Open 3D Engine is, because O3DE's license allows it. Engine names are trademarks of their owners; Render Lab is not
affiliated with or endorsed by any engine's maker.
